use crate::comment::{CommentFromCohost, CommentPermission, InnerComment};
use crate::context::CohostContext;
use crate::data::DbDataError;
use crate::post::{LimitedVisibilityReason, PostAstMap, PostFromCohost, PostState};
use crate::project::ProjectFromCohost;
use crate::render::{PostRenderRequest, PostRenderer};
use crate::Config;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{response, Router};
use chrono::Utc;
use diesel::result::Error as DieselError;
use diesel::SqliteConnection;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tera::{Context, Tera};
use thiserror::Error;
use tokio::sync::Mutex;

struct ServerState {
    ctx: CohostContext,
    tera: Tera,
    post_renderer: PostRenderer,
}

type SharedServerState = Arc<ServerState>;

pub async fn serve(config: Config, db: SqliteConnection) {
    let ctx = CohostContext::new("".into(), PathBuf::from(config.root_dir), Mutex::new(db));

    let mut tera = Tera::new("templates/*").unwrap();
    let post_renderer = PostRenderer::new(4);

    let routes = Router::new()
        .route("/:project/post/:post", get(get_single_post))
        .route("/api/post/:post", get(api_get_post))
        .with_state(Arc::new(ServerState {
            ctx,
            tera,
            post_renderer,
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
    let post = get_cohost_post(&state.ctx, 0, post)
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
    get_single_post_impl(&state, &project, &post)
        .await
        .map_err(|e| render_error_page(&state, e.status(), format!("{e}")).into())
}

fn render_error_page(state: &ServerState, status: StatusCode, message: String) -> Response {
    let mut template_ctx = Context::new();
    template_ctx.insert("message", &message);

    let Ok(body) = state.tera.render("error.html", &template_ctx) else {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::new("failed to render error".to_string()))
            .unwrap();
    };

    Response::builder()
        .status(status)
        .header("content-type", "text/html; charset=utf-8")
        .body(Body::new(body))
        .unwrap()
}

#[derive(Debug, Error)]
enum GetSinglePostError {
    #[error("invalid post ID")]
    InvalidPostId,
    #[error("post not found")]
    PostNotFound,
    #[error("error loading comments: {0}")]
    Comments(GetDataError),
    #[error("error rendering post {0}: {1}")]
    Render(u64, anyhow::Error),
    #[error(transparent)]
    Unknown(anyhow::Error),
}

impl GetSinglePostError {
    fn status(&self) -> StatusCode {
        match self {
            GetSinglePostError::InvalidPostId => StatusCode::BAD_REQUEST,
            GetSinglePostError::PostNotFound => StatusCode::NOT_FOUND,
            GetSinglePostError::Comments(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GetSinglePostError::Render(..) => StatusCode::INTERNAL_SERVER_ERROR,
            GetSinglePostError::Unknown(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

async fn get_single_post_impl(
    state: &ServerState,
    project: &str,
    post: &str,
) -> Result<Response, GetSinglePostError> {
    let post_id = post
        .split('-')
        .next()
        .and_then(|id| id.parse().ok())
        .ok_or(GetSinglePostError::InvalidPostId)?;

    let post = match get_cohost_post(&state.ctx, 0, post_id).await {
        Ok(post) => post,
        Err(GetDataError::NotFound) => return Err(GetSinglePostError::PostNotFound),
        Err(err) => return Err(GetSinglePostError::Unknown(err.into())),
    };

    if post.posting_project.handle != project {
        return Err(GetSinglePostError::PostNotFound);
    }

    let comments = match get_cohost_comments_for_share_tree(&state.ctx, 0, &post).await {
        Ok(comments) => comments,
        Err(err) => return Err(GetSinglePostError::Comments(err.into())),
    };

    let mut rendered_posts = HashMap::new();

    for post in std::iter::once(&post).chain(post.share_tree.iter()) {
        let result = state
            .post_renderer
            .render_post(PostRenderRequest {
                blocks: post.blocks.clone(),
                published_at: post
                    .published_at
                    .clone()
                    .unwrap_or_else(|| Utc::now().to_rfc3339()),
                has_cohost_plus: post.has_cohost_plus,
                disable_embeds: true,
                external_links_in_new_tab: true,
            })
            .await
            .map_err(|e| GetSinglePostError::Render(post.post_id, e))?;

        rendered_posts.insert(post.post_id, result);
    }

    let mut template_ctx = Context::new();
    template_ctx.insert("post", &post);
    template_ctx.insert("comments", &comments);
    template_ctx.insert("rendered_posts", &rendered_posts);

    let body = state
        .tera
        .render("single_post.html", &template_ctx)
        .map_err(|e| GetSinglePostError::Unknown(e.into()))?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .body(Body::new(body))
        .unwrap())
}

#[derive(Debug, Error)]
enum GetDataError {
    #[error("not found")]
    NotFound,
    #[error(transparent)]
    OtherQuery(DieselError),
    #[error("data error: {0}")]
    DbData(#[from] DbDataError),
}

impl From<DieselError> for GetDataError {
    fn from(value: DieselError) -> Self {
        match value {
            DieselError::NotFound => Self::NotFound,
            value => Self::OtherQuery(value),
        }
    }
}

async fn get_cohost_project(
    ctx: &CohostContext,
    viewer_id: u64,
    project_id: u64,
) -> Result<ProjectFromCohost, GetDataError> {
    let project = ctx.project(project_id).await?;

    let project_data = project.data()?;

    Ok(ProjectFromCohost {
        ask_settings: project_data.ask_settings,
        avatar_preview_url: project_data.avatar_preview_url,
        avatar_shape: project_data.avatar_shape,
        avatar_url: project_data.avatar_url,
        contact_card: project_data.contact_card,
        dek: project_data.dek,
        delete_after: project_data.delete_after,
        description: project_data.description,
        display_name: project_data.display_name,
        flags: project_data.flags,
        frequently_used_tags: project_data.frequently_used_tags,
        handle: project.handle,
        header_preview_url: project_data.header_preview_url,
        header_url: project_data.header_url,
        is_self_project: Some(project_id == viewer_id),
        logged_out_post_visibility: project_data.logged_out_post_visibility,
        privacy: project_data.privacy,
        project_id,
        pronouns: project_data.pronouns,
        url: project_data.url,
    })
}

#[async_recursion::async_recursion]
async fn get_cohost_post(
    ctx: &CohostContext,
    viewer_id: u64,
    post_id: u64,
) -> Result<PostFromCohost, GetDataError> {
    // while this could be made more efficient,
    let post = ctx.post(post_id).await?;

    let mut share_tree = Vec::new();
    // this adds extra transparent shares, but whatever
    if let Some(share_post) = post.share_of_post_id {
        let mut post = get_cohost_post(ctx, viewer_id, share_post as u64).await?;
        let post_share_tree = std::mem::replace(&mut post.share_tree, Vec::new());
        share_tree.push(post);
        for post in post_share_tree.into_iter().rev() {
            share_tree.push(post);
        }
    }
    share_tree.reverse();

    let transparent_share_of_post_id = if post.is_transparent_share {
        share_tree
            .iter()
            .rfind(|post| post.transparent_share_of_post_id.is_none())
            .map(|post| post.post_id)
    } else {
        None
    };

    let is_liked = ctx.is_liked(viewer_id, post_id).await?;

    let posting_project =
        get_cohost_project(ctx, viewer_id, post.posting_project_id as u64).await?;

    let post_data = post.data()?;

    let tags = ctx.get_post_tags(post_id).await?;

    Ok(PostFromCohost {
        // we do not use the AST map
        ast_map: PostAstMap {
            read_more_index: None,
            spans: Default::default(),
        },
        blocks: post_data.blocks,
        can_publish: false,
        can_share: !post_data.shares_locked,
        comments_locked: post_data.comments_locked,
        contributor_block_incoming_or_outgoing: false,
        cws: post_data.cws,
        effective_adult_content: post_data.effective_adult_content,
        filename: post.filename,
        has_any_contributor_muted: false,
        has_cohost_plus: post_data.has_cohost_plus,
        headline: post_data.headline,
        is_editor: false,
        is_liked,
        limited_visibility_reason: LimitedVisibilityReason::None,
        num_comments: post_data.num_comments,
        num_shared_comments: post_data.num_shared_comments,
        pinned: post_data.pinned,
        plain_text_body: post_data.plain_text_body,
        post_edit_url: post_data.post_edit_url,
        post_id,
        posting_project,
        published_at: post.published_at,
        related_projects: Default::default(),
        response_to_ask_id: post.response_to_ask_id,
        share_of_post_id: post.share_of_post_id.map(|i| i as u64),
        share_tree,
        shares_locked: post_data.shares_locked,
        single_post_page_url: post_data.single_post_page_url,
        state: PostState::Published,
        tags,
        transparent_share_of_post_id,
    })
}

async fn get_cohost_comments_for_share_tree(
    ctx: &CohostContext,
    viewer_id: u64,
    post: &PostFromCohost,
) -> Result<HashMap<u64, Vec<CommentFromCohost>>, GetDataError> {
    let mut comments = HashMap::with_capacity(post.share_tree.len() + 1);

    comments.insert(
        post.post_id,
        get_cohost_comments(ctx, viewer_id, post.post_id, post.is_editor).await?,
    );

    for post in &post.share_tree {
        comments.insert(
            post.post_id,
            get_cohost_comments(ctx, viewer_id, post.post_id, post.is_editor).await?,
        );
    }

    Ok(comments)
}

async fn get_cohost_comments(
    ctx: &CohostContext,
    viewer_id: u64,
    post_id: u64,
    is_editor: bool,
) -> Result<Vec<CommentFromCohost>, GetDataError> {
    let comments = ctx.get_comments(post_id).await?;

    let mut projects = HashMap::new();
    for comment in &comments {
        if let Some(project) = comment.posting_project_id {
            let project = project as u64;
            if !projects.contains_key(&project) {
                projects.insert(project, get_cohost_project(ctx, viewer_id, project).await?);
            }
        }
    }

    type ByParent = HashMap<String, Vec<CommentFromCohost>>;
    let mut by_parent: ByParent = HashMap::new();
    for comment in comments {
        let comment_data = comment.data()?;

        let is_viewer_comment = comment
            .posting_project_id
            .map_or(false, |p| p as u64 == viewer_id);

        let cohost_comment = CommentFromCohost {
            poster: comment
                .posting_project_id
                .and_then(|proj| projects.get(&(proj as u64)).cloned()),
            comment: InnerComment {
                body: comment_data.body,
                comment_id: comment.id.clone(),
                children: Vec::new(),
                deleted: comment_data.deleted,
                has_cohost_plus: comment_data.has_cohost_plus,
                hidden: comment_data.hidden,
                in_reply_to: comment.in_reply_to_id.clone(),
                post_id,
                posted_at_iso: "".to_string(),
            },
            can_edit: if is_viewer_comment {
                CommentPermission::Allowed
            } else {
                CommentPermission::NotAllowed
            },
            can_hide: if is_editor {
                CommentPermission::Allowed
            } else {
                CommentPermission::NotAllowed
            },
            can_interact: CommentPermission::Allowed,
        };
        by_parent
            .entry(comment.in_reply_to_id.unwrap_or_default())
            .or_default()
            .push(cohost_comment);
    }

    fn collect(by_parent: &mut ByParent, parent: &str) -> Vec<CommentFromCohost> {
        let mut comments = Vec::new();

        if let Some(items) = by_parent.remove(parent) {
            comments.reserve(items.len());

            for mut item in items {
                item.comment.children = collect(by_parent, &item.comment.comment_id);
                comments.push(item);
            }
        }

        comments
    }

    let mut comments = collect(&mut by_parent, "");

    // comments without parents? I dunno
    for items in by_parent.into_values() {
        comments.extend(items);
    }

    Ok(comments)
}
