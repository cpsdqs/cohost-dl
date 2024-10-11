use crate::comment::CommentFromCohost;
use crate::context::{CohostContext, GetError};
use crate::dl::CurrentStateV1;
use crate::feed::TagRelationship;
use crate::post::{PostBlock, PostFromCohost};
use crate::project::{
    AvatarShape, LoggedOutPostVisibility, ProjectAskSettings, ProjectContactCard, ProjectFlag,
    ProjectFromCohost, ProjectPrivacy,
};
use crate::res_ref::ResourceRefs;
use crate::trpc::{LoginLoggedIn, SinglePost};
use anyhow::Context;
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::{Insertable, RunQueryDsl};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::str;
use thiserror::Error;
use tokio::sync::Mutex;

pub struct Database {
    db: Mutex<SqliteConnection>,
}

/// Select fields from posts to store in the database blob
#[derive(Debug, Serialize, Deserialize)]
pub struct PostDataV2 {
    pub blocks: Vec<PostBlock>,
    pub comments_locked: bool,
    pub shares_locked: bool,
    pub cws: Vec<String>,
    pub has_cohost_plus: bool,
    pub headline: String,
    pub num_comments: u64,
    pub num_shared_comments: u64,
    pub plain_text_body: String,
    pub post_edit_url: String,
    pub single_post_page_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostDataV1 {
    pub blocks: Vec<PostBlock>,
    pub comments_locked: bool,
    pub shares_locked: bool,
    pub cws: Vec<String>,
    pub effective_adult_content: bool,
    pub has_cohost_plus: bool,
    pub headline: String,
    pub num_comments: u64,
    pub num_shared_comments: u64,
    pub pinned: bool,
    pub plain_text_body: String,
    pub post_edit_url: String,
    pub single_post_page_url: String,
}

/// Select fields from projects, same deal
#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectDataV1 {
    pub ask_settings: ProjectAskSettings,
    pub avatar_preview_url: String,
    pub avatar_shape: AvatarShape,
    pub avatar_url: String,
    pub contact_card: Vec<ProjectContactCard>,
    pub dek: String,
    pub delete_after: Option<String>,
    pub description: String,
    pub display_name: String,
    pub flags: Vec<ProjectFlag>,
    pub frequently_used_tags: Vec<String>,
    pub header_preview_url: Option<String>,
    pub header_url: Option<String>,
    pub logged_out_post_visibility: LoggedOutPostVisibility,
    pub privacy: ProjectPrivacy,
    pub pronouns: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommentDataV1 {
    pub body: String,
    pub deleted: bool,
    pub has_cohost_plus: bool,
    pub hidden: bool,
}

impl PostDataV2 {
    pub fn from_post(post: &PostFromCohost) -> Self {
        Self {
            blocks: post.blocks.clone(),
            comments_locked: post.comments_locked,
            shares_locked: post.shares_locked,
            cws: post.cws.clone(),
            has_cohost_plus: post.has_cohost_plus,
            headline: post.headline.clone(),
            num_comments: post.num_comments,
            num_shared_comments: post.num_shared_comments,
            plain_text_body: post.plain_text_body.clone(),
            post_edit_url: post.post_edit_url.clone(),
            single_post_page_url: post.single_post_page_url.clone(),
        }
    }

    fn from_v1(data: PostDataV1) -> Self {
        Self {
            blocks: data.blocks,
            comments_locked: data.comments_locked,
            shares_locked: data.shares_locked,
            cws: data.cws,
            has_cohost_plus: data.has_cohost_plus,
            headline: data.headline,
            num_comments: data.num_comments,
            num_shared_comments: data.num_shared_comments,
            plain_text_body: data.plain_text_body,
            post_edit_url: data.post_edit_url,
            single_post_page_url: data.single_post_page_url,
        }
    }
}

impl ProjectDataV1 {
    pub fn from_project(project: &ProjectFromCohost) -> Self {
        Self {
            ask_settings: project.ask_settings.clone(),
            avatar_preview_url: project.avatar_preview_url.clone(),
            avatar_shape: project.avatar_shape,
            avatar_url: project.avatar_url.clone(),
            contact_card: project.contact_card.clone(),
            dek: project.dek.clone(),
            delete_after: project.delete_after.clone(),
            description: project.description.clone(),
            display_name: project.display_name.clone(),
            flags: project.flags.clone(),
            frequently_used_tags: project.frequently_used_tags.clone(),
            header_preview_url: project.header_preview_url.clone(),
            header_url: project.header_url.clone(),
            logged_out_post_visibility: project.logged_out_post_visibility,
            privacy: project.privacy,
            pronouns: project.pronouns.clone(),
            url: project.url.clone(),
        }
    }
}

impl CommentDataV1 {
    pub fn from_comment(comment: &CommentFromCohost) -> Self {
        Self {
            body: comment.comment.body.clone(),
            deleted: comment.comment.deleted,
            has_cohost_plus: comment.comment.has_cohost_plus,
            hidden: comment.comment.hidden,
        }
    }
}

#[derive(Queryable, Insertable, AsChangeset)]
#[diesel(table_name = crate::schema::posts)]
pub struct DbPost {
    pub id: i32,
    pub posting_project_id: i32,
    pub published_at: Option<String>,
    pub response_to_ask_id: Option<String>,
    pub share_of_post_id: Option<i32>,
    pub is_transparent_share: bool,
    pub filename: String,
    pub data: Vec<u8>,
    pub data_version: i32,
    pub state: i32,
    pub is_adult_content: bool,
    pub is_pinned: bool,
}

#[derive(Queryable, Insertable, AsChangeset)]
#[diesel(table_name = crate::schema::projects)]
pub struct DbProject {
    pub id: i32,
    pub handle: String,
    pub is_private: bool,
    pub requires_logged_in: bool,
    pub data: Vec<u8>,
    pub data_version: i32,
}

#[derive(Queryable, Insertable, AsChangeset)]
#[diesel(table_name = crate::schema::comments)]
pub struct DbComment {
    pub id: String,
    pub post_id: i32,
    pub in_reply_to_id: Option<String>,
    pub posting_project_id: Option<i32>,
    pub published_at: String,
    pub data: Vec<u8>,
    pub data_version: i32,
}

#[derive(Debug, Error)]
pub enum DbDataError {
    #[error(transparent)]
    Serde(#[from] rmp_serde::decode::Error),
    #[error("unknown data version {0}")]
    Version(i32),
}

impl DbPost {
    fn from_post(post: &PostFromCohost, data: Vec<u8>, data_version: i32) -> Self {
        Self {
            id: post.post_id as i32,
            posting_project_id: post.posting_project.project_id as i32,
            published_at: post.published_at.clone(),
            response_to_ask_id: post.response_to_ask_id.clone(),
            share_of_post_id: post.share_of_post_id.map(|i| i as i32),
            is_transparent_share: post.transparent_share_of_post_id.is_some(),
            filename: post.filename.clone(),
            data,
            data_version,
            state: post.state as i32,
            is_adult_content: post.effective_adult_content,
            is_pinned: post.pinned,
        }
    }

    pub fn data(&self) -> Result<PostDataV2, DbDataError> {
        if self.data_version == 2 {
            Ok(rmp_serde::from_slice(&self.data)?)
        } else if self.data_version == 1 {
            Ok(PostDataV2::from_v1(rmp_serde::from_slice(&self.data)?))
        } else {
            Err(DbDataError::Version(self.data_version))
        }
    }
}

impl DbProject {
    fn from_project(project: &ProjectFromCohost, data: Vec<u8>, data_version: i32) -> Self {
        Self {
            id: project.project_id as i32,
            handle: project.handle.clone(),
            is_private: project.privacy == ProjectPrivacy::Private,
            requires_logged_in: project.logged_out_post_visibility == LoggedOutPostVisibility::None,
            data,
            data_version,
        }
    }

    pub fn data(&self) -> Result<ProjectDataV1, DbDataError> {
        if self.data_version == 1 {
            Ok(rmp_serde::from_slice(&self.data)?)
        } else {
            Err(DbDataError::Version(self.data_version))
        }
    }
}

impl DbComment {
    fn from_comment(
        post_id: u64,
        comment: &CommentFromCohost,
        data: Vec<u8>,
        data_version: i32,
    ) -> Self {
        Self {
            id: comment.comment.comment_id.clone(),
            post_id: post_id as i32,
            in_reply_to_id: comment.comment.in_reply_to.clone(),
            posting_project_id: comment.poster.as_ref().map(|p| p.project_id as i32),
            published_at: comment.comment.posted_at_iso.clone(),
            data,
            data_version,
        }
    }

    pub fn data(&self) -> Result<CommentDataV1, DbDataError> {
        if self.data_version == 1 {
            Ok(rmp_serde::from_slice(&self.data)?)
        } else {
            Err(DbDataError::Version(self.data_version))
        }
    }
}

impl Database {
    pub fn new(conn: SqliteConnection) -> Self {
        Self {
            db: Mutex::new(conn),
        }
    }

    pub async fn vacuum(&self) -> anyhow::Result<()> {
        Ok(self.db.lock().await.batch_execute("vacuum;")?)
    }
}

/// Project queries
impl Database {
    pub async fn followed_by_any(&self) -> anyhow::Result<Vec<u64>> {
        use crate::schema::follows::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        let result: Vec<i32> = follows.select(to_project_id).get_results(db)?;
        Ok(result.into_iter().map(|i| i as u64).collect())
    }

    pub async fn project(&self, project_id: u64) -> QueryResult<DbProject> {
        use crate::schema::projects::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        Ok(projects.filter(id.eq(project_id as i32)).first(db)?)
    }

    pub async fn project_for_handle(&self, project_handle: &str) -> anyhow::Result<DbProject> {
        use crate::schema::projects::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        Ok(projects.filter(handle.eq(project_handle)).first(db)?)
    }

    pub async fn project_id_for_handle(&self, project_handle: &str) -> QueryResult<u64> {
        use crate::schema::projects::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        let result: i32 = projects
            .filter(handle.eq(project_handle))
            .select(id)
            .first(db)?;
        Ok(result as u64)
    }

    pub async fn has_project_handle(&self, project_handle: &str) -> anyhow::Result<bool> {
        use crate::schema::projects::dsl::*;
        let mut db = self.db.lock().await;
        let db = &mut *db;

        let count: i64 = projects
            .filter(handle.eq(project_handle))
            .count()
            .get_result(db)?;
        Ok(count > 0)
    }

    pub async fn get_all_project_handles_with_posts(&self) -> QueryResult<Vec<String>> {
        use crate::schema::posts::dsl as posts;
        use crate::schema::projects::dsl as projects;
        let mut db = self.db.lock().await;
        let db = &mut *db;

        projects::projects
            .filter(projects::id.eq_any(posts::posts.select(posts::posting_project_id)))
            .order_by(projects::handle)
            .select(projects::handle)
            .load(db)
    }
}

/// Post queries
impl Database {
    pub async fn has_post(&self, post_id: u64) -> QueryResult<bool> {
        use crate::schema::posts::dsl::*;
        let mut db = self.db.lock().await;
        let db = &mut *db;

        let count: i64 = posts.filter(id.eq(post_id as i32)).count().get_result(db)?;
        Ok(count > 0)
    }

    pub async fn is_liked(&self, project_id: u64, post_id: u64) -> QueryResult<bool> {
        use crate::schema::likes::dsl::*;
        let mut db = self.db.lock().await;
        let db = &mut *db;

        let count: i64 = likes
            .filter(from_project_id.eq(project_id as i32))
            .filter(to_post_id.eq(post_id as i32))
            .count()
            .get_result(db)?;
        Ok(count > 0)
    }

    pub async fn post(&self, post_id: u64) -> QueryResult<DbPost> {
        use crate::schema::posts::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        Ok(posts.filter(id.eq(post_id as i32)).first(db)?)
    }

    pub async fn total_non_transparent_post_count(&self) -> anyhow::Result<u64> {
        use crate::schema::posts::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        let result: i64 = posts
            .filter(is_transparent_share.eq(false))
            .count()
            .get_result(db)?;
        Ok(result as u64)
    }

    /// Returns (project, post) tuples
    pub async fn get_post_ids_non_transparent(
        &self,
        offset: i64,
        limit: i64,
    ) -> anyhow::Result<Vec<(u64, u64)>> {
        use crate::schema::posts::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;
        let items = posts
            .filter(is_transparent_share.eq(false))
            .select((posting_project_id, id))
            .offset(offset)
            .limit(limit)
            .load_iter::<(i32, i32), _>(db)?;

        let mut res_items = Vec::new();
        for item in items {
            let (a, b) = item?;
            res_items.push((a as u64, b as u64));
        }
        Ok(res_items)
    }

    pub async fn bad_transparent_shares(&self) -> QueryResult<Vec<u64>> {
        use crate::schema::posts::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;
        let items: Vec<i32> = posts
            .filter(is_transparent_share.eq(true))
            .filter(share_of_post_id.is_null())
            .select(id)
            .load(db)?;
        Ok(items.into_iter().map(|i| i as u64).collect())
    }

    pub async fn is_bad_transparent_share(&self, post_id: u64) -> QueryResult<bool> {
        use crate::schema::posts::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;
        let items: i64 = posts
            .filter(is_transparent_share.eq(true))
            .filter(share_of_post_id.is_null())
            .filter(id.eq(post_id as i32))
            .count()
            .get_result(db)?;
        Ok(items > 0)
    }

    pub async fn all_shares_of_post(&self, post_id: u64) -> QueryResult<Vec<u64>> {
        use crate::schema::posts::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        let items = posts
            .filter(share_of_post_id.eq(post_id as i32))
            .select(id)
            .load_iter::<i32, _>(db)?;

        let mut res_items = Vec::new();
        for item in items {
            res_items.push(item? as u64);
        }
        Ok(res_items)
    }

    pub async fn get_post_tags(&self, the_post_id: u64) -> QueryResult<Vec<String>> {
        use crate::schema::post_tags::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        let items = post_tags
            .filter(post_id.eq(the_post_id as i32))
            .order_by(pos.asc())
            .select(tag)
            .get_results(db)?;

        Ok(items)
    }

    pub async fn posting_project(&self, post_id: u64) -> anyhow::Result<u64> {
        use crate::schema::posts::dsl::*;
        let mut db = self.db.lock().await;
        let db = &mut *db;

        let project_id: i32 = posts
            .filter(id.eq(post_id as i32))
            .select(posting_project_id)
            .first(db)?;
        Ok(project_id as u64)
    }

    pub async fn posting_project_handle(&self, post_id: u64) -> anyhow::Result<(u64, String)> {
        use crate::schema::posts::dsl as posts;
        use crate::schema::projects::dsl as projects;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        let (id, handle): (i32, String) = projects::projects
            .inner_join(posts::posts)
            .filter(posts::id.eq(post_id as i32))
            .select((projects::id, projects::handle))
            .first(db)?;

        Ok((id as u64, handle))
    }

    pub async fn get_comments(&self, the_post_id: u64) -> QueryResult<Vec<DbComment>> {
        use crate::schema::comments::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        comments
            .filter(post_id.eq(the_post_id as i32))
            .order_by(published_at)
            .load(db)
    }
}

#[derive(Debug)]
pub struct PostQuery {
    pub posting_project_id: Option<u64>,
    pub share_of_post_id: Option<u64>,
    pub is_liked_by: Option<u64>,
    pub include_tags: Vec<String>,
    pub exclude_tags: Vec<String>,
    pub is_ask: Option<bool>,
    pub is_adult: Option<bool>,
    pub is_reply: Option<bool>,
    pub is_share: Option<bool>,
    pub is_pinned: Option<bool>,
    pub offset: u64,
    pub limit: u64,
}

impl Default for PostQuery {
    fn default() -> Self {
        Self {
            posting_project_id: None,
            share_of_post_id: None,
            is_liked_by: None,
            include_tags: Vec::new(),
            exclude_tags: Vec::new(),
            is_ask: None,
            is_adult: None,
            is_reply: None,
            is_share: None,
            is_pinned: None,
            offset: 0,
            limit: 20,
        }
    }
}

impl PostQuery {
    fn build(
        &self,
    ) -> diesel::internal::table_macro::BoxedSelectStatement<
        diesel::sql_types::Integer,
        diesel::internal::table_macro::FromClause<crate::schema::posts::table>,
        diesel::sqlite::Sqlite,
    > {
        use crate::schema::likes::dsl as likes;
        use crate::schema::post_tags::dsl as tags;
        use crate::schema::posts::dsl as posts;

        let mut query = posts::posts
            .order_by(posts::published_at.desc())
            .into_boxed();

        if let Some(posting_project_id) = self.posting_project_id {
            query = query.filter(posts::posting_project_id.eq(posting_project_id as i32));
        }
        if let Some(share_of_post_id) = self.share_of_post_id {
            query = query.filter(posts::share_of_post_id.eq(share_of_post_id as i32));
        }

        if !self.include_tags.is_empty() {
            let tagged_posts = tags::post_tags
                .filter(tags::tag.eq_any(self.include_tags.clone()))
                .filter(tags::tag.ne_all(self.exclude_tags.clone()))
                .select(tags::post_id);

            query = query.filter(posts::id.eq_any(tagged_posts));
        } else if !self.exclude_tags.is_empty() {
            let tagged_posts = tags::post_tags
                .filter(tags::tag.ne_all(self.exclude_tags.clone()))
                .select(tags::post_id);

            query = query.filter(posts::id.eq_any(tagged_posts));
        }

        if let Some(is_liked_by) = self.is_liked_by {
            let likes = likes::likes
                .filter(likes::from_project_id.eq(is_liked_by as i32))
                .select(likes::to_post_id);
            query = query.filter(posts::id.eq_any(likes));
        }

        if let Some(is_ask) = self.is_ask {
            if is_ask {
                query = query.filter(posts::response_to_ask_id.is_not_null());
            } else {
                query = query.filter(posts::response_to_ask_id.is_null());
            }
        }

        if let Some(is_reply) = self.is_reply {
            if is_reply {
                query = query
                    .filter(posts::is_transparent_share.eq(false))
                    .filter(posts::share_of_post_id.is_not_null());
            } else {
                query = query.filter(
                    posts::is_transparent_share
                        .eq(true)
                        .or(posts::share_of_post_id.is_null()),
                );
            }
        }

        if let Some(is_share) = self.is_share {
            query = query.filter(posts::is_transparent_share.eq(is_share));
        }

        if let Some(is_adult) = self.is_adult {
            query = query.filter(posts::is_adult_content.eq(is_adult));
        }
        if let Some(is_pinned) = self.is_pinned {
            query = query.filter(posts::is_pinned.eq(is_pinned));
        }

        query.select(posts::id)
    }

    pub async fn get(&self, db: &Database) -> QueryResult<Vec<u64>> {
        let mut db = db.db.lock().await;
        let db = &mut *db;
        let items: Vec<i32> = self
            .build()
            .offset(self.offset as i64)
            .limit((self.limit as i64).min(100))
            .load(db)?;
        Ok(items.into_iter().map(|i| i as u64).collect())
    }

    pub async fn count(&self, db: &Database) -> QueryResult<u64> {
        let mut db = db.db.lock().await;
        let db = &mut *db;
        let count: i64 = self.build().count().get_result(db)?;
        Ok(count as u64)
    }
}

/// Resource queries
impl Database {
    pub async fn total_post_resources_count(&self) -> anyhow::Result<u64> {
        use crate::schema::post_resources::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        let result: i64 = post_resources.count().get_result(db)?;
        Ok(result as u64)
    }

    /// Returns (post, url) tuples
    pub async fn get_post_resources(
        &self,
        offset: i64,
        limit: i64,
    ) -> anyhow::Result<Vec<(u64, String)>> {
        use crate::schema::post_resources::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;
        let items = post_resources
            .select((post_id, url))
            .offset(offset)
            .limit(limit)
            .load_iter::<(i32, String), _>(db)?;

        let mut res_items = Vec::new();
        for item in items {
            let (a, b) = item?;
            res_items.push((a as u64, b));
        }
        Ok(res_items)
    }

    pub async fn total_project_resources_count(&self) -> anyhow::Result<u64> {
        use crate::schema::project_resources::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        let result: i64 = project_resources.count().get_result(db)?;
        Ok(result as u64)
    }

    /// Returns (project, url) tuples
    pub async fn get_project_resources(
        &self,
        offset: i64,
        limit: i64,
    ) -> anyhow::Result<Vec<(u64, String)>> {
        use crate::schema::project_resources::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;
        let items = project_resources
            .select((project_id, url))
            .offset(offset)
            .limit(limit)
            .load_iter::<(i32, String), _>(db)?;

        let mut res_items = Vec::new();
        for item in items {
            let (a, b) = item?;
            res_items.push((a as u64, b));
        }
        Ok(res_items)
    }

    pub async fn total_comment_resources_count(&self) -> anyhow::Result<u64> {
        use crate::schema::comment_resources::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        let result: i64 = comment_resources.count().get_result(db)?;
        Ok(result as u64)
    }

    /// Returns (comment, url) tuples
    pub async fn get_comment_resources(
        &self,
        offset: i64,
        limit: i64,
    ) -> anyhow::Result<Vec<(String, String)>> {
        use crate::schema::comment_resources::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;
        let items = comment_resources
            .select((comment_id, url))
            .offset(offset)
            .limit(limit)
            .load(db)?;
        Ok(items)
    }

    pub async fn get_res_content_type(&self, the_url: &Url) -> anyhow::Result<Option<String>> {
        use crate::schema::resource_content_types::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        let the_url = the_url.to_string();

        let res: Option<String> = resource_content_types
            .filter(url.eq(the_url))
            .select(content_type)
            .first(db)
            .optional()?;

        Ok(res)
    }

    pub async fn get_saved_resource_urls_for_post(&self, post: u64) -> QueryResult<Vec<String>> {
        use crate::schema::post_resources::dsl as res;
        use crate::schema::url_files::dsl as files;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        res::post_resources
            .inner_join(files::url_files.on(res::url.eq(files::url)))
            .filter(res::post_id.eq(post as i32))
            .select(res::url)
            .load(db)
    }

    pub async fn get_saved_resource_urls_for_project(
        &self,
        project: u64,
    ) -> QueryResult<Vec<String>> {
        use crate::schema::project_resources::dsl as res;
        use crate::schema::url_files::dsl as files;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        res::project_resources
            .inner_join(files::url_files.on(res::url.eq(files::url)))
            .filter(res::project_id.eq(project as i32))
            .select(res::url)
            .load(db)
    }

    pub async fn get_saved_resource_urls_for_comment(
        &self,
        comment: &str,
    ) -> QueryResult<Vec<String>> {
        use crate::schema::comment_resources::dsl as res;
        use crate::schema::url_files::dsl as files;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        res::comment_resources
            .inner_join(files::url_files.on(res::url.eq(files::url)))
            .filter(res::comment_id.eq(comment))
            .select(res::url)
            .load(db)
    }

    pub async fn total_url_file_count(&self) -> QueryResult<u64> {
        use crate::schema::url_files::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        let result: i64 = url_files.count().get_result(db)?;
        Ok(result as u64)
    }

    fn read_url_file_path(path: &[u8]) -> QueryResult<PathBuf> {
        // use diesel errors so we can keep using QueryResult
        use diesel::result::Error;

        #[derive(Debug)]
        struct InvalidUrlFilePath;
        impl std::fmt::Display for InvalidUrlFilePath {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "invalid URL file path in database")
            }
        }
        impl std::error::Error for InvalidUrlFilePath {}

        if path.get(0) != Some(&b'@') || path.get(1) != Some(&b'/') {
            return Err(Error::DeserializationError(Box::new(InvalidUrlFilePath)));
        }
        let path = str::from_utf8(path).map_err(|e| Error::DeserializationError(Box::new(e)))?;

        Ok(PathBuf::from(&path[2..]))
    }

    pub async fn get_url_file(&self, the_url: &Url) -> QueryResult<Option<PathBuf>> {
        use crate::schema::url_files::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        let path: Option<Vec<u8>> = url_files
            .filter(url.eq(the_url.to_string()))
            .select(file_path)
            .first(db)
            .optional()?;

        if let Some(path) = path {
            Ok(Some(Self::read_url_file_path(&path)?))
        } else {
            Ok(None)
        }
    }

    pub async fn get_url_files_batch(
        &self,
        offset: i64,
        limit: i64,
    ) -> QueryResult<Vec<(String, PathBuf)>> {
        use crate::schema::url_files::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;
        let items = url_files
            .select((url, file_path))
            .offset(offset)
            .limit(limit)
            .load_iter::<(String, Vec<u8>), _>(db)?;

        let mut res_items = Vec::new();
        for item in items {
            let (the_url, path) = item?;
            res_items.push((the_url, Self::read_url_file_path(&path)?));
        }
        Ok(res_items)
    }
}

/// Insertions
impl Database {
    pub async fn insert_project(&self, project: &ProjectFromCohost) -> anyhow::Result<()> {
        trace!("insert_project {}", project.project_id);

        use crate::schema::project_resources::dsl::*;
        use crate::schema::projects::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        let base = Url::parse(&format!("https://cohost.org/{}", project.handle))?;

        let project_data = ProjectDataV1::from_project(&project);
        let refs = project_data.collect_refs(&base);

        let project_data = rmp_serde::to_vec_named(&project_data).context("DB data")?;

        let db_project = DbProject::from_project(project, project_data, 1);

        diesel::insert_into(projects)
            .values(&db_project)
            .on_conflict(id)
            .do_update()
            .set(&db_project)
            .execute(db)
            .context("DB:projects")?;

        {
            diesel::delete(project_resources)
                .filter(project_id.eq(project.project_id as i32))
                .execute(db)
                .context("DB:project_resources clear")?;

            diesel::insert_into(project_resources)
                .values(
                    &refs
                        .into_iter()
                        .map(|u| {
                            (
                                project_id.eq(project.project_id as i32),
                                url.eq(u.to_string()),
                            )
                        })
                        .collect::<Vec<_>>(),
                )
                .execute(db)
                .context("DB:project_resources")?;
        }

        Ok(())
    }

    pub async fn insert_follow(&self, from_project: u64, to_project: u64) -> anyhow::Result<()> {
        use crate::schema::follows::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        diesel::insert_into(follows)
            .values(&(
                from_project_id.eq(from_project as i32),
                to_project_id.eq(to_project as i32),
            ))
            .execute(db)?;

        Ok(())
    }

    #[async_recursion::async_recursion]
    pub async fn insert_post(
        &self,
        ctx: &CohostContext,
        state: &Mutex<CurrentStateV1>,
        login: &LoginLoggedIn,
        post: &PostFromCohost,
        is_share_post: bool,
        maybe_share_of_post: Option<&PostFromCohost>,
    ) -> anyhow::Result<()> {
        trace!(
            "insert_post {} (ST: {} / S: {:?})",
            post.post_id,
            post.share_tree.len(),
            post.share_of_post_id
        );

        for (i, share_post) in post.share_tree.iter().enumerate() {
            let prev_post = i.checked_sub(1).and_then(|i| post.share_tree.get(i));

            self.insert_post(ctx, state, login, share_post, true, prev_post)
                .await
                .with_context(|| {
                    format!(
                        "inserting share tree post {}/{} for {}/{}",
                        share_post.posting_project.handle,
                        share_post.filename,
                        post.posting_project.handle,
                        post.filename,
                    )
                })?;
        }

        self.insert_project(&post.posting_project)
            .await
            .context("inserting posting project")?;

        for proj in &post.related_projects {
            self.insert_project(&proj)
                .await
                .with_context(|| format!("inserting related project {}", proj.handle))?;
        }

        let mut infer_share_post_from_tree = false;

        if let Some(share_post) = post.share_of_post_id {
            if !self.has_post(share_post).await?
                || self.is_bad_transparent_share(share_post).await?
            {
                if is_share_post {
                    // this scenario happens here:
                    // - transparent share
                    //   - share tree:
                    //     - transparent share
                    //     - transparent share <-- omitted by server!
                    //     - original post
                    //
                    // we can't load the share post directly because there's no endpoint for getting a
                    // post from just the ID. the share post will be present in *this* post's share
                    // tree, however!

                    debug!(
                        "reloading {}/{} because of additionally required post {share_post:?}",
                        post.posting_project.handle, post.filename
                    );

                    let single_post = ctx
                        .posts_single_post(&post.posting_project.handle, post.post_id)
                        .await;

                    match single_post {
                        Ok(single_post) => {
                            return self
                                .insert_single_post(ctx, state, login, &single_post)
                                .await;
                        }
                        Err(err @ GetError::NotFound(..)) => {
                            warn!("could not load additional post due to 404. skipping!\n{err}");
                        }
                        Err(e) => {
                            return Err(e).context(format!(
                                "additional data for share tree post {}/{}",
                                post.posting_project.handle, post.filename
                            ));
                        }
                    }
                }

                // this share post still isn't in the share tree.
                // no idea what's going on here, but it does happen
                warn!("post {}/{} does not have its shared post {} in its share tree. replacing with last available post",
                        post.posting_project.handle,
                        post.filename,
                        share_post,
                    );

                infer_share_post_from_tree = true;
            }
        }

        self.insert_post_final(login, post, infer_share_post_from_tree, maybe_share_of_post)
            .await
    }

    /// Inserts a post. Requires that all dependencies have already been inserted
    pub async fn insert_post_final(
        &self,
        login: &LoginLoggedIn,
        post: &PostFromCohost,
        infer_share_post_from_tree: bool,
        maybe_share_of_post: Option<&PostFromCohost>,
    ) -> anyhow::Result<()> {
        let shared_post_id = if infer_share_post_from_tree {
            let shared_post_id = post.share_tree.last().map(|post| post.post_id);
            if let Some(shared) = shared_post_id {
                Some(shared)
            } else if let Some(maybe) = maybe_share_of_post {
                // scenario:
                // - actual post
                // - share 1 <- ??? deleted probably
                // - share 2 <- we are here, but cohost has omitted the entire share tree for some reason
                // - share 3 <- we'll instead use the share tree from here
                Some(maybe.post_id)
            } else {
                error!(
                    "bizarre mystery scenario: post {}/{} is a share of nothing at all\n",
                    post.posting_project.handle, post.post_id,
                );
                None
            }
        } else {
            post.share_of_post_id
        };

        trace!(
            "insert_post_final {} (S: {:?}, i: {:?})",
            post.post_id,
            shared_post_id,
            infer_share_post_from_tree,
        );

        use crate::schema::likes::dsl::*;
        use crate::schema::posts::dsl::*;

        let base = Url::parse(&post.single_post_page_url).context("invalid post URL")?;

        let post_data = PostDataV2::from_post(&post);
        let refs = post_data.collect_refs(&base);

        let post_data = rmp_serde::to_vec_named(&post_data).context("DB data")?;

        let mut db_post = DbPost::from_post(&post, post_data, 2);
        db_post.share_of_post_id = shared_post_id.map(|i| i as i32);

        let mut db = self.db.lock().await;
        let db = &mut *db;

        diesel::insert_into(posts)
            .values(&db_post)
            .on_conflict(id)
            .do_update()
            .set(&db_post)
            .execute(db)
            .context("DB:posts")?;

        if post.is_liked {
            diesel::insert_into(likes)
                .values(&(
                    from_project_id.eq(login.project_id as i32),
                    to_post_id.eq(post.post_id as i32),
                ))
                .execute(db)
                .context("DB:likes")?;
        }

        {
            use crate::schema::post_related_projects::dsl::*;
            diesel::delete(post_related_projects)
                .filter(post_id.eq(post.post_id as i32))
                .execute(db)
                .context("DB:post_related_projects clear")?;

            for proj in &post.related_projects {
                diesel::insert_into(post_related_projects)
                    .values(&(
                        post_id.eq(post.post_id as i32),
                        project_id.eq(proj.project_id as i32),
                    ))
                    .execute(db)
                    .context("DB:post_related_projects")?;
            }
        }

        {
            use crate::schema::post_tags::dsl::*;
            diesel::delete(post_tags)
                .filter(post_id.eq(post.post_id as i32))
                .execute(db)
                .context("DB:post_tags clear")?;

            diesel::insert_into(post_tags)
                .values(
                    &post
                        .tags
                        .iter()
                        .enumerate()
                        .map(|(i, t)| {
                            (post_id.eq(post.post_id as i32), tag.eq(t), pos.eq(i as i32))
                        })
                        .collect::<Vec<_>>(),
                )
                .execute(db)
                .context("DB:post_tags")?;
        }

        {
            use crate::schema::post_resources::dsl::*;
            diesel::delete(post_resources)
                .filter(post_id.eq(post.post_id as i32))
                .execute(db)
                .context("DB:post_resources clear")?;

            diesel::insert_into(post_resources)
                .values(
                    &refs
                        .into_iter()
                        .map(|u| (post_id.eq(post.post_id as i32), url.eq(u.to_string())))
                        .collect::<Vec<_>>(),
                )
                .execute(db)
                .context("DB:post_resources")?;
        }

        Ok(())
    }

    pub async fn insert_comment(
        &self,
        on_post_id: u64,
        comment: &CommentFromCohost,
    ) -> anyhow::Result<()> {
        use crate::schema::comments::dsl::*;

        let mut queue = VecDeque::new();
        queue.push_back(comment);

        while let Some(comment) = queue.pop_front() {
            if let Some(project) = &comment.poster {
                self.insert_project(project).await?;
            }

            let mut db = self.db.lock().await;
            let db = &mut *db;

            // close enough...
            let base = Url::parse(&format!(
                "https://cohost.org/undefined/post/{}-undefined",
                comment.comment.post_id
            ))?;

            let comment_data = CommentDataV1::from_comment(comment);
            let refs = comment_data.collect_refs(&base);

            let comment_data = rmp_serde::to_vec_named(&comment_data).context("DB data")?;

            let db_comment = DbComment::from_comment(on_post_id, comment, comment_data, 1);

            diesel::insert_into(comments)
                .values(&db_comment)
                .on_conflict(id)
                .do_update()
                .set(&db_comment)
                .execute(db)
                .context("DB:comments")?;

            {
                use crate::schema::comment_resources::dsl::*;
                diesel::delete(comment_resources)
                    .filter(comment_id.eq(&comment.comment.comment_id))
                    .execute(db)
                    .context("DB:comment_resources clear")?;

                diesel::insert_into(comment_resources)
                    .values(
                        &refs
                            .into_iter()
                            .map(|u| {
                                (
                                    comment_id.eq(&comment.comment.comment_id),
                                    url.eq(u.to_string()),
                                )
                            })
                            .collect::<Vec<_>>(),
                    )
                    .execute(db)
                    .context("DB:comment_resources")?;
            }

            for child in &comment.comment.children {
                queue.push_back(child);
            }
        }

        Ok(())
    }

    pub async fn insert_single_post(
        &self,
        ctx: &CohostContext,
        state: &Mutex<CurrentStateV1>,
        login: &LoginLoggedIn,
        single_post: &SinglePost,
    ) -> anyhow::Result<()> {
        trace!("insert_single_post {}", single_post.post.post_id);

        self.insert_post(ctx, state, login, &single_post.post, false, None)
            .await
            .with_context(|| {
                format!(
                    "inserting single post {}/{}",
                    single_post.post.posting_project.handle, single_post.post.filename
                )
            })?;

        for (&post, comments) in &single_post.comments {
            for comment in comments {
                self.insert_comment(post, comment).await.with_context(|| {
                    format!(
                        "inserting single post comment {}/{}/{}",
                        single_post.post.posting_project.handle,
                        single_post.post.filename,
                        comment.comment.comment_id
                    )
                })?;
            }

            let posting_project = self
                .posting_project(post)
                .await
                .context("DB:posting_project")?;
            let mut state = state.lock().await;
            state
                .projects
                .entry(posting_project)
                .or_default()
                .has_comments
                .insert(post);
        }

        Ok(())
    }

    pub async fn insert_related_tags(
        &self,
        tag1: &str,
        tag2: &str,
        rel: TagRelationship,
    ) -> QueryResult<()> {
        use crate::schema::related_tags::dsl;

        // table has `collate nocase`. I think it's ASCII-only
        let (tag1, tag2) = if tag1.to_ascii_lowercase() < tag2.to_ascii_lowercase() {
            (tag1, tag2)
        } else {
            (tag2, tag1)
        };

        let mut db = self.db.lock().await;
        let db = &mut *db;

        diesel::insert_into(dsl::related_tags)
            .values(&(
                dsl::tag1.eq(tag1),
                dsl::tag2.eq(tag2),
                dsl::is_synonym.eq((rel == TagRelationship::Synonym) as i32),
            ))
            .on_conflict_do_nothing()
            .execute(db)?;

        Ok(())
    }

    pub async fn insert_res_content_type(
        &self,
        the_url: &Url,
        the_content_type: &str,
    ) -> anyhow::Result<()> {
        use crate::schema::resource_content_types::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        let the_url = the_url.to_string();

        diesel::insert_into(resource_content_types)
            .values(&(url.eq(the_url), content_type.eq(the_content_type)))
            .on_conflict(url)
            .do_update()
            .set(content_type.eq(the_content_type))
            .execute(db)?;

        Ok(())
    }

    pub async fn insert_url_file(&self, orig_url: &Url, path: &Path) -> anyhow::Result<()> {
        use crate::schema::url_files::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        let orig_url = orig_url.to_string();
        let path = path.to_str().context("path contains invalid UTF-8")?;
        let path_buf = format!("@/{path}").into_bytes();

        diesel::insert_into(url_files)
            .values(&(url.eq(orig_url), file_path.eq(&path_buf)))
            .on_conflict(url)
            .do_update()
            .set(file_path.eq(&path_buf))
            .execute(db)?;

        Ok(())
    }
}

impl Database {
    pub fn get_migration_state(
        db: &mut SqliteConnection,
        the_name: &str,
    ) -> QueryResult<Option<String>> {
        use crate::schema::data_migration_state::dsl::*;

        data_migration_state
            .filter(name.eq(the_name))
            .select(value)
            .first(db)
            .optional()
    }

    pub fn set_migration_state(
        db: &mut SqliteConnection,
        the_name: &str,
        the_value: &str,
    ) -> QueryResult<()> {
        use crate::schema::data_migration_state::dsl::*;

        diesel::insert_into(data_migration_state)
            .values(&(name.eq(the_name), value.eq(the_value)))
            .on_conflict(name)
            .do_update()
            .set(value.eq(the_value))
            .execute(db)?;

        Ok(())
    }

    fn migrate_old_url_file(path: Vec<u8>) -> anyhow::Result<Vec<u8>> {
        if path.get(0) == Some(&b'@') {
            return Ok(path);
        }

        let path = PathBuf::from(unsafe { OsString::from_encoded_bytes_unchecked(path) });
        let path = path
            .to_str()
            .context("could not migrate file path because it contains invalid UTF-8")?;
        Ok(format!("@/{path}").into_bytes())
    }

    /// Migrate from OSString encoded bytes to UTF-8
    pub fn migrate_old_url_files(db: &mut SqliteConnection) -> anyhow::Result<()> {
        if Self::get_migration_state(db, "url_files")?.as_deref() == Some("1") {
            return Ok(());
        }

        {
            use crate::schema::url_files::dsl as url_files;

            for i in (0..).map(|i| i * 1000) {
                let items: Vec<(String, Vec<u8>)> = url_files::url_files
                    .select((url_files::url, url_files::file_path))
                    .offset(i)
                    .limit(1000)
                    .load(db)?;

                if items.is_empty() {
                    break;
                }

                if i == 0 {
                    info!("Migrating url_files to UTF-8");
                }

                for (url, path) in items {
                    let path = Self::migrate_old_url_file(path)?;

                    diesel::update(url_files::url_files)
                        .filter(url_files::url.eq(url))
                        .set(url_files::file_path.eq(path))
                        .execute(db)?;
                }
            }
        }

        Self::set_migration_state(db, "url_files", "1")?;

        Ok(())
    }

    fn migrate_posts_v2(db: &mut SqliteConnection) -> anyhow::Result<()> {
        use crate::schema::posts::dsl as posts;

        for i in (0..).map(|i| i * 1000) {
            let posts: Vec<DbPost> = posts::posts.offset(i).limit(1000).load(db)?;

            if posts.is_empty() {
                break;
            }

            if i == 0 {
                info!("Migration posts to V2");
            }

            for mut post in posts {
                if post.data_version != 1 {
                    continue;
                }

                let data: PostDataV1 = rmp_serde::from_slice(&post.data)?;

                post.is_adult_content = data.effective_adult_content;
                post.is_pinned = data.pinned;

                let data = PostDataV2::from_v1(data);
                post.data = rmp_serde::to_vec_named(&data)?;
                post.data_version = 2;

                diesel::update(posts::posts)
                    .filter(posts::id.eq(post.id))
                    .set((
                        posts::is_adult_content.eq(post.is_adult_content),
                        posts::is_pinned.eq(post.is_pinned),
                        posts::data.eq(post.data),
                        posts::data_version.eq(post.data_version),
                    ))
                    .execute(db)?;
            }
        }

        Ok(())
    }

    pub fn migrate_posts(db: &mut SqliteConnection) -> anyhow::Result<()> {
        let version = Self::get_migration_state(db, "posts_version")?;
        match version.as_deref() {
            Some("2") => (),
            None => {
                Self::migrate_posts_v2(db)?;
                Self::set_migration_state(db, "posts_version", "2")?;
            }
            _ => panic!("database contains invalid posts_version: {version:?}"),
        }

        Ok(())
    }
}
