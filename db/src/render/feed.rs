use crate::data::{Database, PostQuery};
use crate::post::PostFromCohost;
use crate::render::api_data::{cohost_api_post, cohost_api_project, GetDataError};
use crate::render::md_render::{PostRenderRequest, PostRenderResult};
use crate::render::rewrite::rewrite_projects_in_post;
use crate::render::PageRenderer;
use chrono::Utc;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tera::Context;
use thiserror::Error;

pub struct RenderedPosts {
    pub posts: Vec<PostFromCohost>,
    pub rendered_posts: HashMap<u64, PostRenderResult>,
    pub max_page: u64,
}

impl PageRenderer {
    pub async fn get_rendered_posts(
        &self,
        db: &Database,
        viewer_id: u64,
        post_query: &PostQuery,
    ) -> Result<RenderedPosts, GetDataError> {
        let post_ids = post_query.get(db).await?;

        let total_count = post_query.count(db).await?;

        let max_page = total_count.saturating_sub(1) / 20;

        let mut posts = Vec::with_capacity(post_ids.len());
        let mut rendered_posts = HashMap::with_capacity(post_ids.len());

        for post in post_ids {
            let mut post = cohost_api_post(db, viewer_id, post).await?;

            for post in std::iter::once(&post).chain(post.share_tree.iter()) {
                let resources = db.get_saved_resource_urls_for_post(post.post_id).await?;

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
                    .map_err(|e| GetDataError::Render(e))?;

                rendered_posts.insert(post.post_id, result);
            }

            rewrite_projects_in_post(db, &mut post)
                .await
                .map_err(|e| GetDataError::Render(e))?;

            posts.push(post);
        }

        Ok(RenderedPosts {
            posts,
            rendered_posts,
            max_page,
        })
    }
}

#[derive(Debug, Error)]
pub enum RenderFeedError {
    #[error(transparent)]
    Data(#[from] GetDataError),
    #[error(transparent)]
    Render(#[from] tera::Error),
}

impl RenderFeedError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::Data(GetDataError::NotFound) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagFeedQuery {
    #[serde(default)]
    page: u64,
    #[serde(default = "default_true")]
    show_18_plus_posts: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TagFeedFilterState {
    query: TagFeedQuery,
    on_toggle_18_plus_posts: String,
    on_prev_page: String,
    on_next_page: String,
}

impl TagFeedQuery {
    fn fmt_query(&self) -> String {
        let mut out = Vec::new();

        if self.page > 0 {
            out.push(format!("page={}", self.page));
        }
        if !self.show_18_plus_posts {
            out.push("show18PlusPosts=false".into());
        }

        let mut out = out.join("&");
        if !out.is_empty() {
            out.insert(0, '?');
        }
        out
    }

    fn to_filter_state(self, path: &str, max_page: u64) -> TagFeedFilterState {
        let on_toggle_adult = {
            let q = Self {
                show_18_plus_posts: !self.show_18_plus_posts,
                ..self.clone()
            }
            .fmt_query();
            format!("{path}{q}")
        };

        let on_prev_page = if self.page > 0 {
            let q = Self {
                page: self.page - 1,
                ..self.clone()
            }
            .fmt_query();
            format!("{path}{q}")
        } else {
            "".into()
        };
        let on_next_page = if self.page < max_page {
            let q = Self {
                page: self.page + 1,
                ..self.clone()
            }
            .fmt_query();
            format!("{path}{q}")
        } else {
            "".into()
        };

        TagFeedFilterState {
            query: self,
            on_toggle_18_plus_posts: on_toggle_adult,
            on_prev_page,
            on_next_page,
        }
    }
}

impl PageRenderer {
    pub async fn render_tag_feed(
        &self,
        db: &Database,
        path: &str,
        tag: &str,
        query: TagFeedQuery,
    ) -> Result<String, RenderFeedError> {
        let canon_tag = db
            .canonical_tag_capitalization(tag)
            .await
            .map_err(|e| GetDataError::from(e))?
            .unwrap_or(tag.to_string());

        let synonyms = db
            .synonym_tags(&canon_tag)
            .await
            .map_err(|e| GetDataError::from(e))?;

        let related_tags = db
            .related_tags(&canon_tag, &synonyms)
            .await
            .map_err(|e| GetDataError::from(e))?;

        let post_query = PostQuery {
            offset: query.page * 20,
            limit: 20,
            include_tags: vec![canon_tag.clone()],
            is_adult: match query.show_18_plus_posts {
                true => None,
                false => Some(false),
            },
            ..Default::default()
        };

        let RenderedPosts {
            posts,
            rendered_posts,
            max_page,
        } = self.get_rendered_posts(db, 0, &post_query).await?;

        let mut template_ctx = Context::new();
        template_ctx.insert("tag", &canon_tag);

        template_ctx.insert("synonym_tags", &synonyms);
        template_ctx.insert("related_tags", &related_tags);

        template_ctx.insert("posts", &posts);
        template_ctx.insert("rendered_posts", &rendered_posts);

        template_ctx.insert("filter_state", &query.to_filter_state(path, max_page));

        let body = self.tera.render("tag_feed.html", &template_ctx)?;

        Ok(body)
    }

    pub async fn render_liked_feed(
        &self,
        db: &Database,
        project: &str,
        // just re-use this. it's a subset
        query: TagFeedQuery,
    ) -> Result<String, RenderFeedError> {
        let project_id = db
            .project_id_for_handle(project)
            .await
            .map_err(|e| GetDataError::from(e))?;

        let project = cohost_api_project(db, project_id, project_id).await?;

        let post_query = PostQuery {
            offset: query.page * 20,
            limit: 20,
            is_liked_by: Some(project_id),
            ..Default::default()
        };

        let RenderedPosts {
            posts,
            rendered_posts,
            max_page,
        } = self.get_rendered_posts(db, project_id, &post_query).await?;

        let mut template_ctx = Context::new();
        template_ctx.insert("project", &project);

        template_ctx.insert("posts", &posts);
        template_ctx.insert("rendered_posts", &rendered_posts);

        let path = format!("/{}/liked-posts", project.handle);
        template_ctx.insert("filter_state", &query.to_filter_state(&path, max_page));

        let body = self.tera.render("liked_feed.html", &template_ctx)?;

        Ok(body)
    }

    pub async fn render_dashboard(
        &self,
        db: &Database,
        project: &str,
        // just re-use this. it's a subset
        query: TagFeedQuery,
    ) -> Result<String, RenderFeedError> {
        let project_id = db
            .project_id_for_handle(project)
            .await
            .map_err(|e| GetDataError::from(e))?;

        let project = cohost_api_project(db, project_id, project_id).await?;

        let post_query = PostQuery {
            offset: query.page * 20,
            limit: 20,
            is_dashboard_for: Some(project_id),
            ..Default::default()
        };

        let RenderedPosts {
            posts,
            rendered_posts,
            max_page,
        } = self.get_rendered_posts(db, project_id, &post_query).await?;

        let mut template_ctx = Context::new();
        template_ctx.insert("project", &project);

        template_ctx.insert("posts", &posts);
        template_ctx.insert("rendered_posts", &rendered_posts);

        let path = format!("/{}/dashboard", project.handle);
        template_ctx.insert("filter_state", &query.to_filter_state(&path, max_page));

        let body = self.tera.render("dashboard.html", &template_ctx)?;

        Ok(body)
    }
}
