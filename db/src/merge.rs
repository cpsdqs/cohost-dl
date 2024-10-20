use crate::data::{Database, DbPost};
use crate::dl::long_progress_style;
use diesel::{Connection, SqliteConnection};
use indicatif::ProgressBar;

pub async fn merge(db: &Database, other_db: &str) -> anyhow::Result<()> {
    let other_db = Database::new(SqliteConnection::establish(other_db)?);

    let mut posts_inserted = 0;

    let progress = ProgressBar::new(other_db.total_post_count().await?);
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
                insert_post(db, &other_db, post).await?;
                posts_inserted += 1;
            }
        }
    }

    progress.finish_and_clear();
    info!("posts copied: {posts_inserted}");

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
