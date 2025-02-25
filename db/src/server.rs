use crate::bundled_files::CDL_STATIC;
use crate::data::Database;
use crate::render::api_data::{cohost_api_post, GetDataError};
use crate::render::feed::TagFeedQuery;
use crate::render::project_profile::ProjectProfileQuery;
use crate::render::PageRenderer;
use crate::Config;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{response, Router};
use diesel::SqliteConnection;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use thiserror::Error;
use tokio::fs;
use tokio_util::io::ReaderStream;

pub struct ServerState {
    db: Database,
    root_dir: PathBuf,
    page_renderer: PageRenderer,
}

type SharedServerState = Arc<ServerState>;

pub async fn serve(config: Config, db: SqliteConnection, on_listen: impl FnOnce()) {
    let db = Database::new(db);

    let routes = Router::new()
        .route("/rc/tagged/:tag", get(get_global_tagged))
        .route("/:project/post/:post", get(get_single_post))
        .route("/:project", get(get_profile))
        .route("/:project/tagged/:tag", get(get_profile_tagged))
        .route("/:project/liked-posts", get(get_liked))
        .route("/:project/dashboard", get(get_dashboard))
        .route("/api/post/:post", get(api_get_post))
        .route("/r/:proto/:domain/*url", get(get_resource))
        .route("/r/:proto/:domain/", get(get_resource))
        .route("/r", get(get_resource_url))
        .route("/static/:file", get(get_static))
        .route("/", get(get_index))
        .with_state(Arc::new(ServerState {
            db,
            root_dir: PathBuf::from(config.root_dir),
            page_renderer: PageRenderer::new(),
        }));

    let bind_addr = format!("127.0.0.1:{}", config.server_port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await.unwrap();
    info!("serving: http://{bind_addr}");
    on_listen();
    axum::serve(listener, routes).await.unwrap();
}

#[derive(Debug, Error)]
enum ApiError {
    #[error(transparent)]
    Data(#[from] GetDataError),
    #[error(transparent)]
    Unknown(anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self {
            ApiError::Data(GetDataError::NotFound) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        #[derive(Serialize)]
        struct Error {
            message: String,
        }
        let error = Error {
            message: self.to_string(),
        };
        let error = serde_json::to_string(&error).expect("why");

        Response::builder()
            .status(status)
            .header("content-type", "application/json; charset=utf-8")
            .body(Body::new(error))
            .unwrap()
    }
}

async fn api_get_post(
    State(state): State<SharedServerState>,
    Path(post): Path<u64>,
) -> response::Result<Response> {
    let post = cohost_api_post(&state.db, 0, post)
        .await
        .map_err(ApiError::Data)?;
    let body = serde_json::to_string(&post).map_err(|e| ApiError::Unknown(e.into()))?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json; charset=utf-8")
        .body(Body::new(body))
        .unwrap())
}

async fn get_single_post(
    State(state): State<SharedServerState>,
    Path((project, post)): Path<(String, String)>,
) -> response::Result<Response> {
    let body = state
        .page_renderer
        .render_single_post(&state.db, &project, &post)
        .await
        .map_err(|e| render_error_page(&state, e.status(), format!("{e}")))?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .body(Body::new(body))
        .unwrap())
}

async fn get_global_tagged(
    State(state): State<SharedServerState>,
    uri: Uri,
    Path(tag): Path<String>,
    Query(query): Query<TagFeedQuery>,
) -> response::Result<Response> {
    let body = state
        .page_renderer
        .render_tag_feed(&state.db, uri.path(), &tag, query)
        .await
        .map_err(|e| render_error_page(&state, e.status(), format!("{e}")))?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .body(Body::new(body))
        .unwrap())
}

async fn get_liked(
    State(state): State<SharedServerState>,
    Path(project): Path<String>,
    Query(query): Query<TagFeedQuery>,
) -> response::Result<Response> {
    let body = state
        .page_renderer
        .render_liked_feed(&state.db, &project, query)
        .await
        .map_err(|e| render_error_page(&state, e.status(), format!("{e}")))?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .body(Body::new(body))
        .unwrap())
}

async fn get_dashboard(
    State(state): State<SharedServerState>,
    Path(project): Path<String>,
    Query(query): Query<TagFeedQuery>,
) -> response::Result<Response> {
    let body = state
        .page_renderer
        .render_dashboard(&state.db, &project, query)
        .await
        .map_err(|e| render_error_page(&state, e.status(), format!("{e}")))?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .body(Body::new(body))
        .unwrap())
}

async fn get_profile(
    State(state): State<SharedServerState>,
    Path(project): Path<String>,
    Query(query): Query<ProjectProfileQuery>,
) -> response::Result<Response> {
    let body = state
        .page_renderer
        .render_project_profile(&state.db, &project, query, None)
        .await
        .map_err(|e| render_error_page(&state, e.status(), format!("{e}")))?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .body(Body::new(body))
        .unwrap())
}

async fn get_profile_tagged(
    State(state): State<SharedServerState>,
    Path((project, tag)): Path<(String, String)>,
    Query(query): Query<ProjectProfileQuery>,
) -> response::Result<Response> {
    let body = state
        .page_renderer
        .render_project_profile(&state.db, &project, query, Some(tag))
        .await
        .map_err(|e| render_error_page(&state, e.status(), format!("{e}")))?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .body(Body::new(body))
        .unwrap())
}

async fn get_index(State(state): State<SharedServerState>) -> response::Result<Response> {
    let body = state
        .page_renderer
        .render_index_page(&state.db)
        .await
        .map_err(|e| {
            render_error_page(&state, StatusCode::INTERNAL_SERVER_ERROR, format!("{e}"))
        })?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .body(Body::new(body))
        .unwrap())
}

fn render_error_page(state: &ServerState, status: StatusCode, message: String) -> Response {
    let body = state.page_renderer.render_error_page(&message);

    Response::builder()
        .status(status)
        .header("content-type", "text/html; charset=utf-8")
        .body(Body::new(body))
        .unwrap()
}

async fn get_static(
    State(state): State<SharedServerState>,
    Path(file_name): Path<String>,
    headers: HeaderMap,
) -> Response {
    if cfg!(debug_assertions) {
        // use static directory in debug mode
        let mut cdl_static_file = PathBuf::from("static").join(&file_name);
        if !cdl_static_file.exists() {
            cdl_static_file = PathBuf::from("md-render").join("dist").join(&file_name);
        }

        match fs::metadata(&cdl_static_file).await {
            Ok(metadata) => {
                return serve_static(&state, cdl_static_file, metadata, &headers, None).await;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            Err(e) => {
                error!(
                    "could not read file metadata for {}: {e}",
                    cdl_static_file.display()
                );
                return render_error_page(
                    &state,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "could not read file metadata".into(),
                );
            }
        };
    } else if let Some((file, contents)) = CDL_STATIC.iter().find(|(name, _)| *name == file_name) {
        let etag = format!("{file}-{}", env!("BUILD_COMMIT"));

        for cmp_etag in headers.get_all("if-none-match") {
            if cmp_etag.to_str().map_or(false, |value| value == etag) {
                return Response::builder()
                    .status(StatusCode::NOT_MODIFIED)
                    .body(Body::new(String::new()))
                    .unwrap();
            }
        }

        let ext = file.split('.').skip(1).next();
        let content_type = content_type_for_ext(ext);

        // use bundled in release
        return Response::builder()
            .status(StatusCode::OK)
            .header("etag", etag)
            .header("content-type", content_type)
            .header("content-length", contents.len().to_string())
            .header("cache-control", "max-age=3600, must-revalidate")
            .body(Body::from_stream(ReaderStream::new(io::Cursor::new(
                contents,
            ))))
            .unwrap();
    }

    let cohost_static_file = state.root_dir.join("static").join(&file_name);
    match fs::metadata(&cohost_static_file).await {
        Ok(metadata) => serve_static(&state, cohost_static_file, metadata, &headers, None).await,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            render_error_page(&state, StatusCode::NOT_FOUND, "file not found".into())
        }
        Err(e) => {
            error!(
                "could not read file metadata for {}: {e}",
                cohost_static_file.display()
            );
            render_error_page(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not read file metadata".into(),
            )
        }
    }
}

#[derive(Deserialize)]
struct GetResource {
    q: Option<String>,
    h: Option<String>,
}

async fn get_resource(
    State(state): State<SharedServerState>,
    Path((proto, domain)): Path<(String, String)>,
    uri: Uri,
    Query(query): Query<GetResource>,
    headers: HeaderMap,
) -> Response {
    // cannot use path from Path extractor because it decodes URI components.
    // we want the raw path!
    let path = uri
        .path()
        .split('/')
        .skip(4)
        .fold(String::new(), |acc, i| acc + "/" + i);

    let mut url = match Url::parse(&format!("{proto}://{domain}{path}")) {
        Ok(url) => url,
        Err(e) => return render_error_page(&state, StatusCode::BAD_REQUEST, e.to_string()),
    };

    if let Some(q) = query.q {
        url.set_query(Some(&q));
    }
    if let Some(h) = query.h {
        url.set_fragment(Some(&h));
    }

    get_resource_impl(&state, url, headers).await
}

#[derive(Deserialize)]
struct GetResourceUrl {
    url: String,
}

async fn get_resource_url(
    State(state): State<SharedServerState>,
    Query(query): Query<GetResourceUrl>,
    headers: HeaderMap,
) -> Response {
    let url = match Url::parse(&query.url) {
        Ok(url) => url,
        Err(e) => {
            return render_error_page(&state, StatusCode::BAD_REQUEST, e.to_string());
        }
    };
    get_resource_impl(&state, url, headers).await
}

async fn get_resource_impl(state: &ServerState, url: Url, headers: HeaderMap) -> Response {
    let url_file = match state.db.get_url_file(&url).await {
        Ok(path) => path,
        Err(e) => {
            error!("failed to look up file: {e}");
            return render_error_page(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to look up file".into(),
            );
        }
    };

    let Some(url_file) = url_file else {
        return render_error_page(
            &state,
            StatusCode::NOT_FOUND,
            "no such downloaded file".into(),
        );
    };

    let resolved_path = state.root_dir.join(url_file);
    let metadata = match fs::metadata(&resolved_path).await {
        Ok(m) => m,
        Err(e) => {
            error!(
                "could not read file metadata for {}: {e}",
                resolved_path.display()
            );
            return render_error_page(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not read file metadata".into(),
            );
        }
    };

    let content_type = match state.db.get_res_content_type(&url).await {
        Ok(content_type) => content_type,
        Err(e) => {
            error!("failed to look up content type: {e}");
            return render_error_page(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to look up content type".into(),
            );
        }
    };

    serve_static(
        &state,
        resolved_path,
        metadata,
        &headers,
        content_type.as_deref(),
    )
    .await
}

fn content_type_for_ext(ext: Option<&str>) -> &'static str {
    match ext {
        Some("avif") => "image/avif",
        Some("css") => "text/css; charset=utf-8",
        Some("gif") => "image/gif",
        Some("html") => "text/html; charset=utf-8",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("js") => "application/javascript; charset=utf-8",
        Some("jxl") => "image/jxl",
        Some("m4a") => "audio/mp4",
        Some("mp3") => "audio/mp3",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("wav") => "audio/wav",
        Some("webp") => "image/webp",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

async fn serve_static(
    state: &ServerState,
    resolved_path: PathBuf,
    metadata: std::fs::Metadata,
    headers: &HeaderMap,
    content_type: Option<&str>,
) -> Response {
    let etag = if let Ok(mtime) = metadata.modified() {
        let mut etag = Sha256::new();
        etag.update(resolved_path.as_os_str().as_encoded_bytes());

        if let Ok(dur) = mtime.duration_since(SystemTime::UNIX_EPOCH) {
            etag.update(dur.as_nanos().to_le_bytes());
        } else if let Ok(dur) = SystemTime::UNIX_EPOCH.duration_since(mtime) {
            etag.update([0]);
            etag.update(dur.as_nanos().to_le_bytes());
        }

        let etag = etag.finalize();
        Some(hex::encode(etag))
    } else {
        None
    };

    if let (Some(etag), if_none_match) = (&etag, headers.get_all("if-none-match")) {
        for cmp_etag in if_none_match {
            if cmp_etag.to_str().map_or(false, |value| value == etag) {
                return Response::builder()
                    .status(StatusCode::NOT_MODIFIED)
                    .body(Body::new(String::new()))
                    .unwrap();
            }
        }
    }

    let file = match fs::File::open(&resolved_path).await {
        Ok(f) => f,
        Err(e) => {
            error!("could not read file at {}: {e}", resolved_path.display());
            return render_error_page(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not read file".into(),
            );
        }
    };

    let mut response = Response::builder()
        .status(StatusCode::OK)
        .body(Body::from_stream(ReaderStream::new(file)))
        .unwrap();

    response.headers_mut().insert(
        "content-length",
        HeaderValue::from_str(&format!("{}", metadata.len())).unwrap(),
    );

    if let Some(etag) = etag {
        response
            .headers_mut()
            .insert("etag", HeaderValue::from_str(&etag).unwrap());
        response.headers_mut().insert(
            "cache-control",
            HeaderValue::from_str("max-age=3600, must-revalidate").unwrap(),
        );
    }

    let content_type = if let Some(ty) = content_type {
        ty
    } else {
        let file_ext = resolved_path.extension().map(|e| e.to_string_lossy());
        content_type_for_ext(file_ext.as_deref())
    };
    response
        .headers_mut()
        .insert("content-type", HeaderValue::from_str(content_type).unwrap());

    response
}
