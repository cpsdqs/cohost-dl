use crate::context::CohostContext;
use crate::post::{LimitedVisibilityReason, PostAstMap, PostFromCohost, PostState};
use crate::project::ProjectFromCohost;
use crate::Config;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Response, StatusCode};
use axum::routing::get;
use axum::Router;
use diesel::SqliteConnection;
use serde::Serialize;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::io::ReaderStream;

struct ServerState {
    ctx: CohostContext,
}

type SharedServerState = Arc<ServerState>;

pub async fn serve(config: Config, db: SqliteConnection) {
    let ctx = CohostContext::new("".into(), PathBuf::from(config.root_dir), Mutex::new(db));

    let routes = Router::new()
        .route("/api/:viewer/post/:post", get(api_get_post))
        .with_state(Arc::new(ServerState { ctx }));

    let bind_addr = format!("127.0.0.1:{}", config.server_port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await.unwrap();
    info!("serving: http://{bind_addr}");
    axum::serve(listener, routes).await.unwrap();
}

fn json_result<T: Serialize>(result: anyhow::Result<T>) -> Response<Body> {
    #[derive(Serialize)]
    struct OkWrapper<T> {
        success: bool,
        data: T,
    }

    fn make_err(err: String) -> Response<Body> {
        #[derive(Serialize)]
        struct ErrorWrapper {
            success: bool,
            error: String,
        }
        let err_data = serde_json::to_string(&ErrorWrapper {
            success: false,
            error: err,
        })
        .expect("why");

        Response::builder()
            // TODO: better error codes
            .status(StatusCode::GONE)
            .header("content-type", "application/json; charset=utf-8")
            .body(Body::from_stream(ReaderStream::new(io::Cursor::new(
                err_data,
            ))))
            .unwrap()
    }

    match result {
        Ok(data) => match serde_json::to_string(&OkWrapper {
            success: true,
            data,
        }) {
            Ok(data) => Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json; charset=utf-8")
                .body(Body::from_stream(ReaderStream::new(io::Cursor::new(data))))
                .unwrap(),
            Err(err) => make_err(err.to_string()),
        },
        Err(err) => make_err(err.to_string()),
    }
}

#[axum::debug_handler]
async fn api_get_post(
    State(state): State<SharedServerState>,
    Path((viewer_id, post_id)): Path<(u64, u64)>,
) -> Response<Body> {
    json_result(get_cohost_post(&state.ctx, viewer_id, post_id).await)
}

async fn get_cohost_project(
    ctx: &CohostContext,
    viewer_id: u64,
    project_id: u64,
) -> anyhow::Result<ProjectFromCohost> {
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
) -> anyhow::Result<PostFromCohost> {
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
