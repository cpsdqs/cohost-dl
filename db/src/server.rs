use crate::data::Database;
use crate::render::api_data::{cohost_api_post, GetDataError};
use crate::render::PageRenderer;
use crate::Config;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
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

pub async fn serve(config: Config, db: SqliteConnection) {
    let db = Database::new(db);

    let routes = Router::new()
        .route("/:project/post/:post", get(get_single_post))
        .route("/api/post/:post", get(api_get_post))
        .route("/resource", get(get_resource))
        .route("/static/:file", get(get_static))
        .with_state(Arc::new(ServerState {
            db,
            root_dir: PathBuf::from(config.root_dir),
            page_renderer: PageRenderer::new(),
        }));

    let bind_addr = format!("127.0.0.1:{}", config.server_port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await.unwrap();
    info!("serving: http://{bind_addr}");
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
    let cdl_static_file = PathBuf::from("static").join(&file_name);
    let cohost_static_file = state.root_dir.join("static").join(&file_name);

    let (resolved_path, metadata) = match fs::metadata(&cdl_static_file).await {
        Ok(m) => (cdl_static_file, m),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            match fs::metadata(&cohost_static_file).await {
                Ok(m) => (cohost_static_file, m),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    return render_error_page(
                        &state,
                        StatusCode::NOT_FOUND,
                        "file not found".into(),
                    );
                }
                Err(e) => {
                    error!(
                        "could not read file metadata for {}: {e}",
                        cohost_static_file.display()
                    );
                    return render_error_page(
                        &state,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "could not read file metadata".into(),
                    );
                }
            }
        }
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

    serve_static(&state, resolved_path, metadata, &headers).await
}

#[derive(Deserialize)]
struct GetResource {
    url: String,
}

async fn get_resource(
    State(state): State<SharedServerState>,
    Query(query): Query<GetResource>,
    headers: HeaderMap,
) -> Response {
    let url = match Url::parse(&query.url) {
        Ok(url) => url,
        Err(e) => {
            return render_error_page(&state, StatusCode::BAD_REQUEST, format!("{e}"));
        }
    };

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

    serve_static(&state, resolved_path, metadata, &headers).await
}

async fn serve_static(
    state: &ServerState,
    resolved_path: PathBuf,
    metadata: std::fs::Metadata,
    headers: &HeaderMap,
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

    let file_ext = resolved_path.extension().map(|e| e.to_string_lossy());
    let content_type = match file_ext.as_deref() {
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
    };
    response
        .headers_mut()
        .insert("content-type", HeaderValue::from_str(content_type).unwrap());

    response
}
