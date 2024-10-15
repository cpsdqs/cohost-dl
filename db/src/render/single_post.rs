use crate::comment::CommentFromCohost;
use crate::data::Database;
use crate::render::api_data::{cohost_api_comments_for_share_tree, cohost_api_post, GetDataError};
use crate::render::md_render::{
    MarkdownRenderContext, MarkdownRenderRequest, MarkdownRenderResult, MarkdownRenderer,
    PostRenderRequest,
};
use crate::render::{rewrite, PageRenderer};
use axum::http::StatusCode;
use chrono::Utc;
use std::collections::HashMap;
use std::convert::identity;
use tera::Context;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RenderSinglePostError {
    #[error("invalid post ID")]
    InvalidPostId,
    #[error("post not found")]
    PostNotFound,
    #[error("error loading comments: {0}")]
    Comments(GetDataError),
    #[error("error rendering post {0}: {1}")]
    Render(u64, anyhow::Error),
    #[error("error rendering project: {0}")]
    RenderProject(anyhow::Error),
    #[error("error rendering comment: {0}")]
    RenderComment(anyhow::Error),
    #[error("{0:?}")]
    Unknown(anyhow::Error),
}

impl RenderSinglePostError {
    pub fn status(&self) -> StatusCode {
        match self {
            RenderSinglePostError::InvalidPostId => StatusCode::BAD_REQUEST,
            RenderSinglePostError::PostNotFound => StatusCode::NOT_FOUND,
            RenderSinglePostError::Comments(_) => StatusCode::INTERNAL_SERVER_ERROR,
            RenderSinglePostError::Render(..) => StatusCode::INTERNAL_SERVER_ERROR,
            RenderSinglePostError::RenderProject(..) => StatusCode::INTERNAL_SERVER_ERROR,
            RenderSinglePostError::RenderComment(..) => StatusCode::INTERNAL_SERVER_ERROR,
            RenderSinglePostError::Unknown(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl PageRenderer {
    pub async fn render_single_post(
        &self,
        db: &Database,
        project: &str,
        post: &str,
    ) -> Result<String, RenderSinglePostError> {
        let post_id = post
            .split('-')
            .next()
            .and_then(|id| id.parse().ok())
            .ok_or(RenderSinglePostError::InvalidPostId)?;

        let mut post = match cohost_api_post(db, 0, post_id).await {
            Ok(post) => post,
            Err(GetDataError::NotFound) => return Err(RenderSinglePostError::PostNotFound),
            Err(err) => return Err(RenderSinglePostError::Unknown(err.into())),
        };

        if post.posting_project.handle != project {
            return Err(RenderSinglePostError::PostNotFound);
        }

        rewrite::rewrite_projects_in_post(db, &mut post)
            .await
            .map_err(|e| RenderSinglePostError::Unknown(e))?;

        let mut comments = match cohost_api_comments_for_share_tree(db, 0, &post).await {
            Ok(comments) => comments,
            Err(err) => return Err(RenderSinglePostError::Comments(err.into())),
        };

        for comment in comments.values_mut().flat_map(identity) {
            rewrite::rewrite_projects_in_comment(db, comment)
                .await
                .map_err(|e| RenderSinglePostError::Unknown(e))?;
        }

        let mut rendered_comments = HashMap::new();

        #[async_recursion::async_recursion]
        async fn render_comment(
            db: &Database,
            md: &MarkdownRenderer,
            comment: &CommentFromCohost,
            comments: &mut HashMap<String, MarkdownRenderResult>,
        ) -> Result<(), RenderSinglePostError> {
            let resources = db
                .get_saved_resource_urls_for_comment(&comment.comment.comment_id)
                .await
                .map_err(|e| RenderSinglePostError::Unknown(e.into()))?;

            let result = md
                .render_markdown(MarkdownRenderRequest {
                    markdown: comment.comment.body.clone(),
                    context: MarkdownRenderContext::Comment,
                    published_at: comment.comment.posted_at_iso.clone(),
                    has_cohost_plus: comment.comment.has_cohost_plus,
                    resources,
                })
                .await
                .map_err(|e| RenderSinglePostError::RenderComment(e))?;

            comments.insert(comment.comment.comment_id.clone(), result);

            for child in &comment.comment.children {
                render_comment(db, md, child, comments).await?;
            }
            Ok(())
        }
        for comment in comments.values().flat_map(identity) {
            render_comment(db, &self.md, comment, &mut rendered_comments).await?;
        }

        let mut rendered_posts = HashMap::new();

        for post in std::iter::once(&post).chain(post.share_tree.iter()) {
            let resources = db
                .get_saved_resource_urls_for_post(post.post_id)
                .await
                .map_err(|e| RenderSinglePostError::Unknown(e.into()))?;

            let result = self
                .md
                .render_post(PostRenderRequest {
                    post_id: post.post_id,
                    blocks: post.blocks.clone(),
                    published_at: post
                        .published_at
                        .clone()
                        .unwrap_or_else(|| Utc::now().to_rfc3339()),
                    has_cohost_plus: post.has_cohost_plus,
                    resources,
                })
                .await
                .map_err(|e| RenderSinglePostError::Render(post.post_id, e))?;

            rendered_posts.insert(post.post_id, result);
        }

        let resources = db
            .get_saved_resource_urls_for_project(post.posting_project.project_id)
            .await
            .map_err(|e| RenderSinglePostError::Unknown(e.into()))?;

        let rendered_project_description = self
            .md
            .render_markdown(MarkdownRenderRequest {
                markdown: post.posting_project.description.clone(),
                published_at: Utc::now().to_rfc3339(),
                context: MarkdownRenderContext::Profile,
                has_cohost_plus: false,
                resources,
            })
            .await
            .map_err(|e| RenderSinglePostError::RenderProject(e))?;

        let mut template_ctx = Context::new();
        template_ctx.insert("post", &post);
        template_ctx.insert("comments", &comments);
        template_ctx.insert("rendered_comments", &rendered_comments);
        template_ctx.insert("rendered_posts", &rendered_posts);
        template_ctx.insert(
            "rendered_project_description",
            &rendered_project_description,
        );

        let body = self
            .tera
            .render("single_post.html", &template_ctx)
            .map_err(|e| RenderSinglePostError::Unknown(e.into()))?;

        Ok(body)
    }
}
