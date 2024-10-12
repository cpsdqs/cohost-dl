use crate::comment::{CommentFromCohost, CommentPermission, InnerComment};
use crate::data::{Database, DbDataError};
use crate::post::{LimitedVisibilityReason, PostAstMap, PostFromCohost, PostState};
use crate::project::ProjectFromCohost;
use diesel::result::Error as DieselError;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GetDataError {
    #[error("not found")]
    NotFound,
    #[error(transparent)]
    OtherQuery(DieselError),
    #[error("data error: {0}")]
    DbData(#[from] DbDataError),
    #[error("render error: {0}")]
    Render(anyhow::Error),
}

impl From<DieselError> for GetDataError {
    fn from(value: DieselError) -> Self {
        match value {
            DieselError::NotFound => Self::NotFound,
            value => Self::OtherQuery(value),
        }
    }
}

pub async fn cohost_api_project(
    db: &Database,
    viewer_id: u64,
    project_id: u64,
) -> Result<ProjectFromCohost, GetDataError> {
    let project = db.project(project_id).await?;

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
pub async fn cohost_api_post(
    db: &Database,
    viewer_id: u64,
    post_id: u64,
) -> Result<PostFromCohost, GetDataError> {
    // while this could be made more efficient,
    let post = db.post(post_id).await?;

    let mut share_tree = Vec::new();
    // this adds extra transparent shares, but whatever
    if let Some(share_post) = post.share_of_post_id {
        let mut post = cohost_api_post(db, viewer_id, share_post as u64).await?;
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

    let is_liked = if viewer_id != 0 {
        db.is_liked(viewer_id, post_id).await?
    } else {
        false
    };

    let posting_project = cohost_api_project(db, viewer_id, post.posting_project_id as u64).await?;

    let post_data = post.data()?;

    let tags = db.get_post_tags(post_id).await?;

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
        effective_adult_content: post.is_adult_content,
        filename: post.filename,
        has_any_contributor_muted: false,
        has_cohost_plus: post_data.has_cohost_plus,
        headline: post_data.headline,
        is_editor: false,
        is_liked,
        limited_visibility_reason: LimitedVisibilityReason::None,
        num_comments: post_data.num_comments,
        num_shared_comments: post_data.num_shared_comments,
        pinned: post.is_pinned,
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

pub async fn cohost_api_comments_for_share_tree(
    db: &Database,
    viewer_id: u64,
    post: &PostFromCohost,
) -> Result<HashMap<u64, Vec<CommentFromCohost>>, GetDataError> {
    let mut comments = HashMap::with_capacity(post.share_tree.len() + 1);

    comments.insert(
        post.post_id,
        cohost_api_comments(db, viewer_id, post.post_id, post.is_editor).await?,
    );

    for post in &post.share_tree {
        comments.insert(
            post.post_id,
            cohost_api_comments(db, viewer_id, post.post_id, post.is_editor).await?,
        );
    }

    Ok(comments)
}

pub async fn cohost_api_comments(
    db: &Database,
    viewer_id: u64,
    post_id: u64,
    is_editor: bool,
) -> Result<Vec<CommentFromCohost>, GetDataError> {
    let comments = db.get_comments(post_id).await?;

    let mut projects = HashMap::new();
    for comment in &comments {
        if let Some(project) = comment.posting_project_id {
            let project = project as u64;
            if !projects.contains_key(&project) {
                projects.insert(project, cohost_api_project(db, viewer_id, project).await?);
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
                posted_at_iso: comment.published_at,
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
