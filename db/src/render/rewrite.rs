use crate::comment::CommentFromCohost;
use crate::data::Database;
use crate::post::PostFromCohost;
use crate::project::ProjectFromCohost;

fn make_resource_url(s: &str) -> String {
    format!("/resource?url={}", urlencoding::encode(s))
}

pub async fn rewrite_project(db: &Database, project: &mut ProjectFromCohost) -> anyhow::Result<()> {
    let resources = db
        .get_saved_resource_urls_for_project(project.project_id)
        .await?;

    if resources.contains(&project.avatar_url) {
        project.avatar_url = make_resource_url(&project.avatar_url);
    }
    if resources.contains(&project.avatar_preview_url) {
        project.avatar_preview_url = make_resource_url(&project.avatar_preview_url);
    }
    if let Some(header_url) = &mut project.header_url {
        if resources.contains(header_url) {
            *header_url = make_resource_url(header_url);
        }
    }
    if let Some(header_preview_url) = &mut project.header_preview_url {
        if resources.contains(header_preview_url) {
            *header_preview_url = make_resource_url(header_preview_url);
        }
    }

    Ok(())
}

#[async_recursion::async_recursion]
pub async fn rewrite_projects_in_post(db: &Database, post: &mut PostFromCohost) -> anyhow::Result<()> {
    rewrite_project(db, &mut post.posting_project).await?;

    for post in &mut post.share_tree {
        rewrite_projects_in_post(db, post).await?
    }

    Ok(())
}

#[async_recursion::async_recursion]
pub async fn rewrite_projects_in_comment(
    db: &Database,
    comment: &mut CommentFromCohost,
) -> anyhow::Result<()> {
    if let Some(poster) = &mut comment.poster {
        rewrite_project(db, poster).await?;
    }

    for comment in &mut comment.comment.children {
        rewrite_projects_in_comment(db, comment).await?
    }

    Ok(())
}
