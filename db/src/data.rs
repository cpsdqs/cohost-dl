use crate::comment::CommentFromCohost;
use crate::context::{CohostContext, GetError};
use crate::dl::CurrentStateV1;
use crate::post::{PostBlock, PostFromCohost};
use crate::project::{
    AvatarShape, LoggedOutPostVisibility, ProjectAskSettings, ProjectContactCard, ProjectFlag,
    ProjectFromCohost, ProjectPrivacy,
};
use crate::res_ref::ResourceRefs;
use crate::trpc::{LoginLoggedIn, SinglePost};
use anyhow::{bail, Context};
use diesel::prelude::*;
use diesel::{Insertable, RunQueryDsl};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::Path;
use tokio::sync::Mutex;

/// Select fields from posts to store in the database blob
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

impl PostDataV1 {
    pub fn from_post(post: &PostFromCohost) -> Self {
        Self {
            blocks: post.blocks.clone(),
            comments_locked: post.comments_locked,
            shares_locked: post.shares_locked,
            cws: post.cws.clone(),
            effective_adult_content: post.effective_adult_content,
            has_cohost_plus: post.has_cohost_plus,
            headline: post.headline.clone(),
            num_comments: post.num_comments,
            num_shared_comments: post.num_shared_comments,
            pinned: post.pinned,
            plain_text_body: post.plain_text_body.clone(),
            post_edit_url: post.post_edit_url.clone(),
            single_post_page_url: post.single_post_page_url.clone(),
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
        }
    }

    pub fn data(&self) -> anyhow::Result<PostDataV1> {
        if self.data_version == 1 {
            Ok(rmp_serde::from_slice(&self.data)?)
        } else {
            bail!("unknown data version {}", self.data_version)
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

    pub fn data(&self) -> anyhow::Result<ProjectDataV1> {
        if self.data_version == 1 {
            Ok(rmp_serde::from_slice(&self.data)?)
        } else {
            bail!("unknown data version {}", self.data_version)
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

    pub fn data(&self) -> anyhow::Result<CommentDataV1> {
        if self.data_version == 1 {
            Ok(rmp_serde::from_slice(&self.data)?)
        } else {
            bail!("unknown data version {}", self.data_version)
        }
    }
}

impl CohostContext {
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

    pub async fn followed_by_any(&self) -> anyhow::Result<Vec<u64>> {
        use crate::schema::follows::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        let result: Vec<i32> = follows.select(to_project_id).get_results(db)?;
        Ok(result.into_iter().map(|i| i as u64).collect())
    }

    pub async fn project(&self, project_id: u64) -> anyhow::Result<DbProject> {
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

    pub async fn has_post(&self, post_id: u64) -> anyhow::Result<bool> {
        use crate::schema::posts::dsl::*;
        let mut db = self.db.lock().await;
        let db = &mut *db;

        let count: i64 = posts.filter(id.eq(post_id as i32)).count().get_result(db)?;
        Ok(count > 0)
    }

    pub async fn is_liked(&self, project_id: u64, post_id: u64) -> anyhow::Result<bool> {
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

    pub async fn post(&self, post_id: u64) -> anyhow::Result<DbPost> {
        use crate::schema::posts::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        Ok(posts.filter(id.eq(post_id as i32)).first(db)?)
    }

    pub async fn total_post_count(&self) -> anyhow::Result<u64> {
        use crate::schema::posts::dsl::*;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        let result: i64 = posts.count().get_result(db)?;
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

    pub async fn get_post_tags(&self, the_post_id: u64) -> anyhow::Result<Vec<String>> {
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

    pub async fn posting_project_handle(&self, post_id: u64) -> anyhow::Result<String> {
        use crate::schema::posts::dsl as posts;
        use crate::schema::projects::dsl as projects;

        let mut db = self.db.lock().await;
        let db = &mut *db;

        let project_handle: String = projects::projects
            .inner_join(posts::posts)
            .filter(posts::id.eq(post_id as i32))
            .select(projects::handle)
            .first(db)?;

        Ok(project_handle)
    }

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

    #[async_recursion::async_recursion]
    pub async fn insert_post(
        &self,
        state: &Mutex<CurrentStateV1>,
        login: &LoginLoggedIn,
        post: &PostFromCohost,
        is_share_post: bool,
    ) -> anyhow::Result<()> {
        trace!(
            "insert_post {} (ST: {} / S: {:?})",
            post.post_id,
            post.share_tree.len(),
            post.share_of_post_id
        );

        for share_post in &post.share_tree {
            self.insert_post(state, login, share_post, true)
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
            if !self.has_post(share_post).await? {
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

                    let single_post = self
                        .posts_single_post(&post.posting_project.handle, post.post_id)
                        .await;

                    match single_post {
                        Ok(single_post) => {
                            return self.insert_single_post(state, login, &single_post).await;
                        }
                        Err(err @ GetError::NotFound(..)) => {
                            error!("could not load additional post due to 404. skipping!\n{err}");
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
                error!("post {}/{} does not have its shared post {} in its share tree. replacing with last available post",
                        post.posting_project.handle,
                        post.filename,
                        share_post,
                    );
                infer_share_post_from_tree = true;
            }
        }

        self.insert_post_final(login, post, infer_share_post_from_tree)
            .await
    }

    /// Inserts a post. Requires that all dependencies have already been inserted
    pub async fn insert_post_final(
        &self,
        login: &LoginLoggedIn,
        post: &PostFromCohost,
        infer_share_post_from_tree: bool,
    ) -> anyhow::Result<()> {
        let shared_post_id = if infer_share_post_from_tree {
            post.share_tree.last().map(|post| post.post_id)
        } else {
            post.share_of_post_id
        };

        trace!(
            "insert_post_final {} (S: {:?})",
            post.post_id,
            shared_post_id
        );

        use crate::schema::likes::dsl::*;
        use crate::schema::posts::dsl::*;

        let base = Url::parse(&post.single_post_page_url).context("invalid post URL")?;

        let post_data = PostDataV1::from_post(&post);
        let refs = post_data.collect_refs(&base);

        let post_data = rmp_serde::to_vec_named(&post_data).context("DB data")?;

        let mut db_post = DbPost::from_post(&post, post_data, 1);
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
        state: &Mutex<CurrentStateV1>,
        login: &LoginLoggedIn,
        single_post: &SinglePost,
    ) -> anyhow::Result<()> {
        trace!("insert_single_post {}", single_post.post.post_id);

        self.insert_post(state, login, &single_post.post, false)
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
        let path = path.as_os_str().as_encoded_bytes();

        diesel::insert_into(url_files)
            .values(&(url.eq(orig_url), file_path.eq(path)))
            .on_conflict(url)
            .do_update()
            .set(file_path.eq(path))
            .execute(db)?;

        Ok(())
    }
}
