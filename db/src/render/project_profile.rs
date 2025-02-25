use crate::data::{Database, PostQuery};
use crate::render::api_data::cohost_api_project;
use crate::render::feed::RenderedPosts;
use crate::render::md_render::{MarkdownRenderContext, MarkdownRenderRequest};
use crate::render::rewrite::rewrite_project;
use crate::render::PageRenderer;
use axum::http::StatusCode;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tera::Context;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RenderProjectProfileError {
    #[error("no such project")]
    NoSuchProject,
    #[error("error rendering project: {0}")]
    RenderProject(anyhow::Error),
    #[error("{0:?}")]
    Unknown(anyhow::Error),
}

impl RenderProjectProfileError {
    pub fn status(&self) -> StatusCode {
        match self {
            RenderProjectProfileError::NoSuchProject => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProfileQuery {
    page: Option<u64>,
    #[serde(default)]
    hide_shares: bool,
    #[serde(default)]
    hide_replies: bool,
    #[serde(default)]
    hide_asks: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FilterState {
    query: ProjectProfileQuery,

    on_show_shares: String,
    on_hide_shares: String,
    on_show_replies: String,
    on_hide_replies: String,
    on_show_asks: String,
    on_hide_asks: String,
    on_prev_page: String,
    on_next_page: String,
}

impl ProjectProfileQuery {
    fn fmt_query(&self) -> String {
        let mut out = Vec::new();

        if let Some(page) = self.page {
            out.push(format!("page={page}"));
        }
        if self.hide_shares {
            out.push("hideShares=true".into());
        }
        if self.hide_replies {
            out.push("hideReplies=true".into());
        }
        if self.hide_asks {
            out.push("hideAsks=true".into());
        }

        let mut out = out.join("&");
        if !out.is_empty() {
            out.insert(0, '?');
        }
        out
    }

    #[rustfmt::skip]
    fn to_filter_state(&self, max_page: u64) -> FilterState {
        let on_show_shares = Self { hide_shares: false, ..self.clone() }.fmt_query();
        let on_hide_shares = Self { hide_shares: true, ..self.clone() }.fmt_query();
        let on_show_replies = Self { hide_replies: false, ..self.clone() }.fmt_query();
        let on_hide_replies = Self { hide_replies: true, ..self.clone() }.fmt_query();
        let on_show_asks = Self { hide_asks: false, ..self.clone() }.fmt_query();
        let on_hide_asks = Self { hide_asks: true, ..self.clone() }.fmt_query();

        let page = self.page.unwrap_or_default();
        let on_prev_page = if page > 0 {
            Self { page: Some(page - 1), ..self.clone() }.fmt_query()
        } else {
            "".into()
        };
        let on_next_page = if page < max_page {
            Self { page: Some(page + 1), ..self.clone() }.fmt_query()
        } else {
            "".into()
        };

        FilterState {
            query: self.clone(),
            on_show_shares,
            on_hide_shares,
            on_show_replies,
            on_hide_replies,
            on_show_asks,
            on_hide_asks,
            on_prev_page,
            on_next_page,
        }
    }
}

impl PageRenderer {
    pub async fn render_project_profile(
        &self,
        db: &Database,
        project_handle: &str,
        query: ProjectProfileQuery,
        tagged: Option<String>,
    ) -> Result<String, RenderProjectProfileError> {
        let project_id = db
            .project_id_for_handle(project_handle)
            .await
            .map_err(|e| match e {
                diesel::result::Error::NotFound => RenderProjectProfileError::NoSuchProject,
                e => RenderProjectProfileError::Unknown(e.into()),
            })?;

        let mut project = cohost_api_project(db, 0, project_id)
            .await
            .map_err(|e| RenderProjectProfileError::Unknown(e.into()))?;

        rewrite_project(db, &mut project)
            .await
            .map_err(|e| RenderProjectProfileError::Unknown(e.into()))?;

        let resources = db
            .get_saved_resource_urls_for_project(project_id)
            .await
            .map_err(|e| RenderProjectProfileError::Unknown(e.into()))?;

        let rendered_project_description = self
            .md
            .render_markdown(MarkdownRenderRequest {
                markdown: project.description.clone(),
                published_at: Utc::now().to_rfc3339(),
                context: MarkdownRenderContext::Profile,
                has_cohost_plus: false,
                resources,
            })
            .await
            .map_err(|e| RenderProjectProfileError::RenderProject(e))?;

        let post_query = PostQuery {
            posting_project_id: Some(project_id),
            offset: query.page.unwrap_or_default() * 20,
            limit: 20,
            include_tags: tagged.iter().cloned().collect(),
            is_share: if query.hide_shares { Some(false) } else { None },
            is_reply: if query.hide_replies {
                Some(false)
            } else {
                None
            },
            is_ask: if query.hide_asks { Some(false) } else { None },
            ..Default::default()
        };

        let RenderedPosts {
            posts,
            rendered_posts,
            max_page,
        } = self
            .get_rendered_posts(db, 0, &post_query)
            .await
            .map_err(|e| RenderProjectProfileError::Unknown(e.into()))?;

        let mut template_ctx = Context::new();
        template_ctx.insert("project", &project);
        template_ctx.insert(
            "rendered_project_description",
            &rendered_project_description,
        );
        template_ctx.insert("posts", &posts);
        template_ctx.insert("rendered_posts", &rendered_posts);
        template_ctx.insert("filter_state", &query.to_filter_state(max_page));

        if let Some(tag) = tagged {
            template_ctx.insert("tagged", &tag);
        }

        let body = self
            .tera
            .render("project_profile.html", &template_ctx)
            .map_err(|e| RenderProjectProfileError::Unknown(e.into()))?;

        Ok(body)
    }
}
