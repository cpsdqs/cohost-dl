use crate::data::{Database, DbPost};
use crate::dl::long_progress_style;
use anyhow::{bail, Context};
use deno_core::url::Url;
use diesel::{Connection, SqliteConnection};
use indicatif::ProgressBar;
use std::path::Path;

pub async fn merge(
    db: &Database,
    other_db: &str,
    root_dir: &Path,
    other_root_dir: &Path,
) -> anyhow::Result<()> {
    info!("merging from database at {}", other_db);

    let other_db = Database::new(SqliteConnection::establish(other_db)?);
    let other_total_post_count = other_db.total_post_count().await?;
    info!("checking {other_total_post_count} posts");

    let mut posts_inserted = 0;

    let progress = ProgressBar::new(other_total_post_count);
    progress.set_style(long_progress_style());
    progress.set_message("comparing posts");

    for offset in (0..).map(|i| i * 1000) {
        let posts = other_db.get_post_ids(offset, 1000).await?;
        if posts.is_empty() {
            break;
        }

        for post_id in posts {
            progress.inc(1);
            let post = other_db.post(post_id).await?;

            if db.is_db_post_better_somehow(&post).await? {
                debug!("inserting better post for {post_id}");
                progress.set_message(format!("copying post {post_id}"));
                insert_post(db, &other_db, post).await?;
                posts_inserted += 1;

                progress.set_message("comparing posts");
            }
        }
    }

    progress.finish_and_clear();
    if posts_inserted == 1 {
        info!("1 post copied");
    } else {
        info!("{posts_inserted} posts copied");
    }

    let other_total_comment_count = other_db.total_comment_count().await?;
    info!("checking {other_total_comment_count} comments");

    let mut comment_posts_inserted = 0;

    let progress = ProgressBar::new(other_total_comment_count);
    progress.set_style(long_progress_style());
    progress.set_message("checking comments");

    for offset in (0..).map(|i| i * 1000) {
        let comments = other_db.get_comment_ids(offset, 1000).await?;
        if comments.is_empty() {
            break;
        }

        for comment_id in comments {
            progress.inc(1);

            if !db.has_comment(&comment_id).await? {
                let comment = other_db.comment(&comment_id).await?;
                debug!("inserting comment {comment_id}");

                let post_id = comment.post_id as u64;
                progress.set_message(format!(
                    "copying comments for post {post_id}, including {comment_id}"
                ));

                insert_comments(db, &other_db, post_id)
                    .await
                    .context("inserting comments")?;
                comment_posts_inserted += 1;

                progress.set_message("checking comments");
            }
        }
    }

    progress.finish_and_clear();
    if comment_posts_inserted == 1 {
        info!("comments copied for 1 post");
    } else {
        info!("comments copied for {comment_posts_inserted} posts");
    }

    let other_total_file_count = other_db.total_url_file_count().await?;
    info!("checking {other_total_file_count} files");

    let mut files_inserted = 0;

    let progress = ProgressBar::new(other_total_file_count);
    progress.set_style(long_progress_style());
    progress.set_message("copying files");

    for offset in (0..).map(|i| i * 1000) {
        let files = other_db.get_url_files_batch(offset, 1000).await?;
        if files.is_empty() {
            break;
        }

        for (url, path) in files {
            progress.inc(1);

            let Ok(url) = Url::parse(&url) else { continue };

            if !db.get_url_file(&url).await?.is_some() {
                let from_path = other_root_dir.join(&path);
                let to_path = root_dir.join(&path);
                progress.set_message(format!("{}", path.display()));

                if !from_path.exists() {
                    bail!(
                        "error copying a file because it doesn't exist:\n{}",
                        from_path.display()
                    );
                }
                if to_path.exists() {
                    // probably pointing at the same files directory
                    continue;
                }
                let mut to_path_dir = to_path.clone();
                to_path_dir.pop();
                std::fs::create_dir_all(&to_path_dir)
                    .with_context(|| format!("creating directory for {}", to_path.display()))?;
                std::fs::copy(&from_path, &to_path).with_context(|| {
                    format!(
                        "copying file from {} to {}",
                        from_path.display(),
                        to_path.display()
                    )
                })?;
                db.insert_url_file(&url, &path).await?;

                files_inserted += 1;
                progress.set_message("copying files");
            }
        }
    }

    progress.finish_and_clear();
    if files_inserted == 1 {
        info!("1 file copied");
    } else {
        info!("{files_inserted} files copied");
    }

    info!("Done");

    Ok(())
}

#[async_recursion::async_recursion]
async fn insert_post(db: &Database, other_db: &Database, post: DbPost) -> anyhow::Result<()> {
    let mut share_of_post_id = post.share_of_post_id;

    if let Some(share_of_post) = share_of_post_id {
        let mut insert_share = true;

        if db.has_post(share_of_post as u64).await? {
            let this_post = db.post(share_of_post as u64).await?;
            let other_post = other_db.post(share_of_post as u64).await?;

            if let (Some(this_pub), Some(other_pub)) =
                (&this_post.published_at, &other_post.published_at)
            {
                if this_pub >= other_pub {
                    // probably a better share post
                    share_of_post_id = Some(this_post.id);
                    insert_share = false;
                }
            }
        }

        if insert_share {
            let post = other_db.post(share_of_post as u64).await?;
            insert_post(db, other_db, post).await?;
        }
    }

    if !db.has_project_id(post.posting_project_id as u64).await? {
        let api_project = crate::render::api_data::cohost_api_project(
            other_db,
            0,
            post.posting_project_id as u64,
        )
        .await?;
        db.insert_project(&api_project, true).await?;
    }

    // whatever, this works
    let mut api_post =
        crate::render::api_data::cohost_api_post(other_db, 0, post.id as u64).await?;
    api_post.share_of_post_id = share_of_post_id.map(|i| i as u64);
    db.insert_post_final(&Default::default(), &api_post, false, None)
        .await?;

    Ok(())
}

async fn insert_comments(db: &Database, other_db: &Database, post: u64) -> anyhow::Result<()> {
    let api_comments =
        crate::render::api_data::cohost_api_comments(other_db, 0, post, false).await?;
    for comment in api_comments {
        db.insert_comment(post, &comment, true).await?;
    }
    Ok(())
}
