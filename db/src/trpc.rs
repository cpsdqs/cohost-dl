use crate::comment::CommentFromCohost;
use crate::context::{CohostContext, GetError};
use crate::post::PostFromCohost;
use crate::project::ProjectFromCohost;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginLoggedIn {
    pub activated: bool,
    pub delete_after: Option<String>,
    pub email: String,
    pub email_verified: bool,
    pub email_verify_canceled: bool,
    pub logged_in: bool,
    pub mod_mode: bool,
    pub project_id: u64,
    pub read_only: bool,
    pub two_factor_active: bool,
    pub user_id: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfilePostsInput {
    project_handle: String,
    page: u64,
    options: ProfilePostsOptions,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfilePostsOptions {
    hide_asks: bool,
    hide_replies: bool,
    hide_shares: bool,
    pinned_posts_at_top: bool,
    viewing_on_project_page: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePosts {
    pub pagination: ProfilePostsPagination,
    pub posts: Vec<PostFromCohost>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListEditedProjects {
    pub projects: Vec<ProjectFromCohost>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePostsPagination {
    current_page: u64,
    /// Bogus. do not trust this guy
    more_pages_forward: bool,
    next_page: Option<u64>,
    previous_page: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SinglePostInput {
    handle: String,
    post_id: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SinglePost {
    pub post: PostFromCohost,
    pub comments: HashMap<u64, Vec<CommentFromCohost>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FollowedFeedInput {
    cursor: u64,
    limit: u64,
    before_timestamp: u64,
    sort_order: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowedFeedQuery {
    pub next_cursor: Option<u64>,
    pub projects: Vec<FollowedFeedProject>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowedFeedProject {
    pub project: ProjectFromCohost,
    pub latest_post: Option<PostFromCohost>,
    pub project_pinned: bool,
}

impl CohostContext {
    pub async fn login_logged_in(&self) -> Result<LoginLoggedIn, GetError> {
        self.trpc_query::<(), _>("login.loggedIn", None).await
    }

    pub async fn posts_profile_posts(
        &self,
        project_handle: &str,
        page: u64,
    ) -> Result<ProfilePosts, GetError> {
        let input = ProfilePostsInput {
            project_handle: project_handle.into(),
            page,
            options: ProfilePostsOptions {
                hide_asks: false,
                hide_replies: false,
                hide_shares: false,
                pinned_posts_at_top: true,
                viewing_on_project_page: true,
            },
        };

        self.trpc_query("posts.profilePosts", Some(input)).await
    }

    pub async fn posts_single_post(
        &self,
        project_handle: &str,
        post_id: u64,
    ) -> Result<SinglePost, GetError> {
        let input = SinglePostInput {
            handle: project_handle.into(),
            post_id,
        };

        self.trpc_query("posts.singlePost", Some(input)).await
    }

    pub async fn projects_list_edited_projects(&self) -> Result<ListEditedProjects, GetError> {
        self.trpc_query::<(), _>("projects.listEditedProjects", None)
            .await
    }

    pub async fn projects_by_handle(&self, handle: &str) -> Result<ProjectFromCohost, GetError> {
        self.trpc_query("projects.byHandle", Some(handle)).await
    }

    pub async fn projects_followed_feed_query(
        &self,
        before_timestamp: u64,
        cursor: u64,
        limit: u64,
    ) -> Result<FollowedFeedQuery, GetError> {
        let input = FollowedFeedInput {
            cursor,
            limit,
            before_timestamp,
            sort_order: "alpha-asc".into(),
        };

        self.trpc_query("projects.followedFeed.query", Some(input))
            .await
    }

    pub async fn projects_followed_feed_query_all(
        &self,
    ) -> Result<Vec<FollowedFeedProject>, GetError> {
        let timestamp = Utc::now().timestamp_millis() as u64;

        let mut projects = Vec::new();

        let mut cursor = Some(0);
        while let Some(current_cursor) = cursor {
            let result = self
                .projects_followed_feed_query(timestamp, current_cursor, 20)
                .await?;
            cursor = result.next_cursor;

            projects.extend(result.projects);
        }

        Ok(projects)
    }
}
