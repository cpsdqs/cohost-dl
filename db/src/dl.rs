use crate::bundled_files::COHOST_STATIC;
use crate::context::{CohostContext, GetError, MAX_RETRIES};
use crate::trpc::LoginLoggedIn;
use crate::Config;
use anyhow::{bail, Context};
use diesel::SqliteConnection;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;

#[derive(Debug, Default, Serialize, Deserialize)]
struct CurrentState {
    version: u64,
    data: serde_json::Value,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CurrentStateV1 {
    pub has_likes: HashSet<u64>,
    pub has_follows: HashSet<u64>,
    pub projects: HashMap<u64, ProjectState>,
    pub failed_urls: Vec<String>,
    #[serde(default)]
    pub tagged_posts: HashMap<String, TaggedPostsState>,
    #[serde(default)]
    pub comments_lost_to_time: HashSet<u64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ProjectState {
    pub has_all_posts: bool,
    pub has_comments: HashSet<u64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TaggedPostsState {
    pub has_all_posts: bool,
    pub has_up_to: Option<(u64, u64)>,
}

impl CurrentStateV1 {
    const FILE: &'static str = "downloader-state.json";

    fn load_state() -> anyhow::Result<Self> {
        if fs::exists(Self::FILE)? {
            let s = fs::read_to_string(Self::FILE)?;
            let state: CurrentState = serde_json::from_str(&s)?;
            if state.version != 1 {
                bail!("unknown version {}", state.version);
            }
            Ok(serde_json::from_value(state.data)?)
        } else {
            Ok(Self::default())
        }
    }

    fn store_state(&self) -> anyhow::Result<()> {
        let data = serde_json::to_value(self)?;
        let state = CurrentState { version: 1, data };
        let state = serde_json::to_string_pretty(&state)?;
        fs::write(Self::FILE, state)?;
        Ok(())
    }
}

impl Drop for CurrentStateV1 {
    fn drop(&mut self) {
        self.store_state().unwrap()
    }
}

fn ok_or_quit<T, E>(r: Result<T, E>) -> T
where
    E: Into<anyhow::Error>,
{
    match r {
        Ok(r) => r,
        Err(e) => {
            let e = e.into();
            error!("{e:?}");
            std::process::exit(1);
        }
    }
}

async fn login(ctx: &CohostContext) -> anyhow::Result<LoginLoggedIn> {
    info!("logging in");
    let logged_in = ctx.login_logged_in().await?;
    let edited_projects = ctx.projects_list_edited_projects().await?;

    let current_handle = edited_projects
        .projects
        .iter()
        .find(|p| p.project_id == logged_in.project_id)
        .map(|p| format!("@{}", p.handle))
        .unwrap_or("(error)".into());

    info!("logged in as {} / {}", logged_in.email, current_handle);
    warn!("please do not change your currently active page ({current_handle}) in the cohost web UI while loading data");

    for project in edited_projects.projects {
        ctx.insert_project(&project).await?;
    }

    Ok(logged_in)
}

async fn load_follows(
    ctx: &CohostContext,
    login: &LoginLoggedIn,
    state: &mut CurrentStateV1,
) -> anyhow::Result<()> {
    info!("loading follows for project {}", login.project_id);
    let followed = ctx.projects_followed_feed_query_all().await?;
    info!("loaded follows: {}", followed.len());

    for f in followed {
        ctx.insert_project(&f.project).await?;
        ctx.insert_follow(login.project_id, f.project.project_id)
            .await?;
    }

    state.has_follows.insert(login.project_id);

    Ok(())
}

async fn load_likes(
    ctx: &CohostContext,
    login: &LoginLoggedIn,
    state: &Mutex<CurrentStateV1>,
) -> anyhow::Result<()> {
    let mut ref_timestamp = None;
    let mut skip_posts = 0;

    info!("loading liked posts for project {}", login.project_id);

    let bar = ProgressBar::new_spinner();
    bar.enable_steady_tick(Duration::from_millis(100));

    let mut page = 0;
    let mut count = 0;
    let mut has_next = true;
    while has_next {
        let feed = ctx.load_liked_posts(ref_timestamp, skip_posts).await?;

        page += 1;
        count += feed.posts.len();
        bar.set_message(format!("liked posts page {page} ({count} posts)"));

        skip_posts += feed.pagination_mode.ideal_page_stride;
        ref_timestamp = Some(feed.pagination_mode.ref_timestamp);
        has_next = feed.pagination_mode.more_pages_forward;

        for post in &feed.posts {
            ctx.insert_post(ctx, state, login, post, false, None)
                .await?;
        }
    }

    bar.finish_and_clear();

    info!("loaded liked posts: {count}");
    state.lock().await.has_likes.insert(login.project_id);

    Ok(())
}

async fn load_profile_posts(
    ctx: &CohostContext,
    state: &Mutex<CurrentStateV1>,
    login: &LoginLoggedIn,
    project_id: u64,
    new_only: bool,
) -> anyhow::Result<()> {
    let project = ctx.project(project_id).await?;

    if new_only {
        info!("loading new posts from @{}", project.handle);
    } else {
        info!("loading all posts from @{}", project.handle);
    }

    let bar = ProgressBar::new_spinner();
    bar.enable_steady_tick(Duration::from_millis(100));
    bar.set_message(format!("@{} first page", project.handle));

    let mut count = 0;
    'outer: for page in 0.. {
        let posts = ctx.posts_profile_posts(&project.handle, page).await?;

        let message = format!("@{} page {} ({count} posts)", project.handle, page + 1);
        bar.set_message(message.clone());

        for (i, post) in posts.posts.iter().enumerate() {
            if new_only && ctx.has_post(post.post_id).await? {
                if post.pinned {
                    // Placing this continue in this exact spot so that it:
                    // - will download new pinned posts
                    // - will not stop early on an old pinned post; there might be new posts after
                    // - will not count old pinned posts towards the "new posts loaded" count
                    continue;
                }

                break 'outer;
            }

            bar.set_message(format!(
                "{message} ← adding post {i}/{} (ID {})",
                posts.posts.len(),
                post.post_id
            ));

            ctx.insert_post(ctx, state, login, post, false, None)
                .await?;
            count += 1;
        }

        if posts.posts.is_empty() {
            break;
        }
    }

    bar.finish_and_clear();

    info!("loaded all posts from @{}: {count}", project.handle);
    state
        .lock()
        .await
        .projects
        .get_mut(&project_id)
        .unwrap()
        .has_all_posts = true;

    Ok(())
}

async fn load_tagged_posts(
    ctx: &CohostContext,
    state: &Mutex<CurrentStateV1>,
    login: &LoginLoggedIn,
    tag: &str,
) -> anyhow::Result<()> {
    info!("loading all posts tagged with #{tag}");

    let bar = ProgressBar::new_spinner();
    bar.enable_steady_tick(Duration::from_millis(100));
    bar.set_message(format!("#{tag} first page"));

    let saved_state = state
        .lock()
        .await
        .tagged_posts
        .entry(tag.to_string())
        .or_default()
        .has_up_to;

    let (mut ref_timestamp, mut skip_posts) = if let Some(state) = saved_state {
        (Some(state.0), state.1)
    } else {
        (None, 0)
    };

    let mut has_related_tags = false;
    let mut canonical_tag = None;

    let mut count = 0;
    let mut has_next = true;
    while has_next {
        let feed = ctx
            .load_tagged_posts(tag, ref_timestamp, skip_posts)
            .await?;

        if !has_related_tags {
            let canonical = feed
                .synonyms_and_related_tags
                .iter()
                .find(|item| item.content.to_lowercase() == tag.to_lowercase());
            let canonical = if let Some(canonical) = canonical {
                canonical_tag = Some(canonical.content.clone());
                canonical.content.clone()
            } else {
                warn!("could not determine canonical capitalization of #{tag}");
                tag.to_string()
            };

            for item in feed.synonyms_and_related_tags {
                if item.content == canonical {
                    // self relationship is always synonym
                    continue;
                }

                ctx.insert_related_tags(&canonical, &item.content, item.relationship)
                    .await
                    .with_context(|| format!("inserting related tag for #{tag}"))?;
            }
            has_related_tags = true;
        }

        let display_tag = canonical_tag.as_deref().unwrap_or(tag);

        skip_posts += feed.pagination_mode.ideal_page_stride;
        ref_timestamp = Some(feed.pagination_mode.ref_timestamp);
        has_next = feed.pagination_mode.more_pages_forward;

        let message = format!("#{display_tag} offset {} ({count} posts)", skip_posts);
        bar.set_message(message.clone());

        for (i, post) in feed.posts.iter().enumerate() {
            bar.set_message(format!(
                "{message} ← adding post {i}/{} (ID {})",
                feed.posts.len(),
                post.post_id
            ));

            ctx.insert_post(ctx, state, login, post, false, None)
                .await?;
        }

        state
            .lock()
            .await
            .tagged_posts
            .entry(tag.to_string())
            .or_default()
            .has_up_to = Some((feed.pagination_mode.ref_timestamp, skip_posts));

        count += feed.posts.len();
        if feed.posts.is_empty() {
            break;
        }
    }

    bar.finish_and_clear();

    info!("loaded all posts tagged with #{tag}: {count}");
    state
        .lock()
        .await
        .tagged_posts
        .entry(tag.to_string())
        .or_default()
        .has_all_posts = true;

    Ok(())
}

fn long_progress_style() -> ProgressStyle {
    ProgressStyle::with_template("{bar:40} {pos:>7}/{len:7} (eta: {eta}) {msg}")
        .unwrap()
        .progress_chars("▓▒░ ")
}

async fn posts_without_comments(
    ctx: &CohostContext,
    state: &CurrentStateV1,
) -> anyhow::Result<Vec<u64>> {
    let mut posts_without_comments = Vec::new();

    let total_count = ctx.total_non_transparent_post_count().await?;
    let progress = ProgressBar::new(total_count);
    progress.set_style(long_progress_style());

    progress.set_message("checking posts");

    let mut offset = 0;
    loop {
        progress.set_position(offset as u64);

        let posts = ctx.get_post_ids_non_transparent(offset, 1000).await?;

        offset += 1000;
        if posts.is_empty() {
            break;
        }

        posts_without_comments.extend(
            posts
                .into_iter()
                .filter(|(proj, post)| {
                    state
                        .projects
                        .get(proj)
                        .map_or(true, |proj| !proj.has_comments.contains(post))
                        && !state.comments_lost_to_time.contains(post)
                })
                .map(|(_, post)| post),
        );
    }

    progress.finish_and_clear();

    Ok(posts_without_comments)
}

async fn load_comments_for_posts(
    ctx: &CohostContext,
    state: &Mutex<CurrentStateV1>,
    login: &LoginLoggedIn,
    mut posts: Vec<u64>,
) -> anyhow::Result<()> {
    let progress = ProgressBar::new(posts.len() as u64);
    progress.set_style(long_progress_style());

    posts.sort();

    // share -> original post
    let mut share_map: HashMap<u64, u64> = HashMap::new();
    // original post -> shares
    let mut rev_share_map: HashMap<u64, HashSet<u64>> = HashMap::new();

    // use reverse ID order because later shares might have comments for earlier posts, saving time
    let mut count = 0;
    while let Some(post) = posts.pop() {
        let for_post = share_map.get(&post).copied().unwrap_or(post);

        let is_last = if let Some(items) = rev_share_map.get_mut(&for_post) {
            items.remove(&post);
            items.is_empty()
        } else {
            true
        };

        progress.inc(1);
        let (project_id, for_project_handle) = ctx.posting_project_handle(for_post).await?;

        let already_has_comments = state
            .lock()
            .await
            .projects
            .entry(project_id)
            .or_default()
            .has_comments
            .contains(&for_post);

        if already_has_comments {
            trace!("skipping {post}/{for_post} because we already have comments, probably from a share");
            continue;
        }

        let (_, project_handle) = ctx.posting_project_handle(post).await?;

        if for_post == post {
            progress.set_message(format!("{project_handle}/{post}"));
        } else {
            progress.set_message(format!(
                "{for_project_handle}/{for_post} from {project_handle}/{post}"
            ));
        }

        match ctx.posts_single_post(&project_handle, post).await {
            Ok(post) => {
                ctx.insert_single_post(ctx, state, login, &post).await?;
                count += 1;
            }
            Err(GetError::NotFound(..)) => {
                let shares = ctx.all_shares_of_post(post).await?;

                if shares.is_empty() {
                    if is_last {
                        state.lock().await.comments_lost_to_time.insert(for_post);
                        error!("comments for {for_project_handle}/{for_post} are lost to time");
                    }
                } else {
                    for share in shares {
                        share_map.insert(share, for_post);
                        rev_share_map.entry(for_post).or_default().insert(share);
                        posts.push(share);
                        progress.inc_length(1);
                    }
                }
            }
            Err(e) => Err(e)?,
        }
    }

    progress.finish_and_clear();

    if count == 1 {
        info!("loaded comments for 1 post");
    } else {
        info!("loaded comments for {count} posts");
    }

    Ok(())
}

async fn fix_bad_transparent_shares(
    ctx: &CohostContext,
    state: &Mutex<CurrentStateV1>,
    login: &LoginLoggedIn,
) -> anyhow::Result<()> {
    let bad_transparent_shares = ctx.bad_transparent_shares().await?;

    if bad_transparent_shares.is_empty() {
        return Ok(());
    }

    let progress = ProgressBar::new(bad_transparent_shares.len() as u64);
    progress.set_style(long_progress_style());

    progress.set_message("fixing transparent shares");

    let mut fixed = 0;

    for post in bad_transparent_shares {
        progress.inc(1);

        let mut shares = ctx.all_shares_of_post(post).await?;

        let mut was_maybe_fixed = false;
        while let Some(share) = shares.pop() {
            progress.set_message(format!(
                "fixing transparent share {post} (trying share {share})"
            ));

            let (_, share_post_handle) = ctx.posting_project_handle(share).await?;

            match ctx.posts_single_post(&share_post_handle, share).await {
                Ok(post) => {
                    ctx.insert_single_post(ctx, state, login, &post).await?;
                    trace!("fixed with post {}", post.post.post_id);
                    was_maybe_fixed = true;
                    fixed += 1;
                    break;
                }
                Err(GetError::NotFound(..)) => {
                    let share_shares = ctx.all_shares_of_post(share).await?;
                    shares.extend(share_shares);
                }
                Err(e) => Err(e)?,
            }
        }

        if !was_maybe_fixed {
            error!("post {post} is a transparent share of nothing\n");
        } else {
            let post = ctx.post(post).await?;
            let fixed = post.is_transparent_share && !post.share_of_post_id.is_none();
            if !fixed {
                error!("could not fix transparent share {} because ????\n", post.id);
            }
        }
    }

    progress.finish_and_clear();

    info!("fixed transparent shares: {fixed}");

    Ok(())
}

async fn par_load_resources<T: Send + Sync>(
    ctx: &CohostContext,
    state: &Mutex<CurrentStateV1>,
    loaded: &Arc<AtomicU64>,
    items: &[T],
    error_id: impl Fn(&T) -> String + Send + Copy,
    get_url: impl Fn(&T) -> &str,
) -> anyhow::Result<()> {
    /// don’t want this unsafe {} block to be too large, so this is a “safe” wrapper
    async fn unsafe_scope<'a, T, R, F>(
        f: F,
    ) -> (
        R,
        Vec<<async_scoped::spawner::use_tokio::Tokio as async_scoped::spawner::Spawner<T>>::FutureOutput>,
    )
    where
        T: Send + 'static,
        F: FnOnce(
            &mut async_scoped::Scope<'a, T, async_scoped::spawner::use_tokio::Tokio>,
        ) -> R,
    {
        unsafe { async_scoped::TokioScope::scope_and_collect(f).await }
    }

    // SAFETY: the future is not forgotten
    let (res, results) = unsafe_scope(|s| -> anyhow::Result<()> {
        for item in items {
            let url = Url::parse(&get_url(item))?;

            let loaded = Arc::clone(&loaded);
            s.spawn(async move {
                let mut did_something = false;

                let mut tries = 0;
                let res = loop {
                    tries += 1;

                    let res = ctx
                        .load_resource_to_file(&url, &state, Some(&mut did_something))
                        .await;

                    match res {
                        Ok(res) => break Ok(res),
                        Err(e) if e.is_recoverable() && tries < MAX_RETRIES => {
                            let wait = 1.8_f64.powf(tries as f64) - 1.;
                            info!(
                                "try {} for {}: waiting for {wait:.02}s before continuing to be polite",
                                tries + 1,
                                url,
                            );
                            sleep(Duration::from_secs_f64(wait)).await;
                        }
                        Err(e) => break Err(e),
                    }
                };

                match res {
                    Ok(Some(path)) => match ctx.insert_url_file(&url, &path).await {
                        Ok(()) => {
                            if did_something {
                                loaded.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        Err(e) => {
                            error!(
                                "resource for {}: could not save URL mapping: {e}",
                                error_id(item)
                            );
                        }
                    },
                    Ok(None) => (),
                    Err(e) => {
                        error!("resource for {}: {e}", error_id(item));
                    }
                }
            });
        }

        Ok(())
    })
    .await;

    res?;
    for res in results {
        res?;
    }

    Ok(())
}

const RESOURCE_LOAD_BATCH_SIZE: u64 = 5;

async fn load_cohost_resources(
    ctx: &CohostContext,
    state: &Mutex<CurrentStateV1>,
) -> anyhow::Result<()> {
    let files: Vec<_> = COHOST_STATIC
        .lines()
        .filter(|line| !line.is_empty())
        .collect();

    let progress = ProgressBar::new(files.len() as u64);
    progress.set_style(long_progress_style());
    progress.set_message("loading static files");

    let loaded = Arc::new(AtomicU64::new(0));

    for (i, chunk) in files.chunks(RESOURCE_LOAD_BATCH_SIZE as usize).enumerate() {
        progress.set_position(i as u64 * RESOURCE_LOAD_BATCH_SIZE);
        par_load_resources(ctx, state, &loaded, chunk, |url| url.to_string(), |url| url).await?;
    }

    progress.finish_and_clear();

    let loaded = loaded.load(Ordering::Acquire);
    if loaded == 1 {
        info!("loaded 1 static file");
    } else if loaded > 0 {
        info!("loaded {loaded} static files");
    }

    Ok(())
}

async fn load_post_resources(
    ctx: &CohostContext,
    state: &Mutex<CurrentStateV1>,
) -> anyhow::Result<()> {
    let total = ctx.total_post_resources_count().await?;

    info!("checking post resource files");

    let progress = ProgressBar::new(total);
    progress.set_style(long_progress_style());

    let loaded = Arc::new(AtomicU64::new(0));

    let pages = total.next_multiple_of(RESOURCE_LOAD_BATCH_SIZE) / RESOURCE_LOAD_BATCH_SIZE;
    for i in 0..pages {
        progress.set_position(i * RESOURCE_LOAD_BATCH_SIZE);

        let items = ctx
            .get_post_resources(
                i as i64 * RESOURCE_LOAD_BATCH_SIZE as i64,
                RESOURCE_LOAD_BATCH_SIZE as i64,
            )
            .await?;

        par_load_resources(
            ctx,
            state,
            &loaded,
            &items,
            |(post, _)| format!("post {post}"),
            |(_, url)| url,
        )
        .await?;
    }

    progress.finish_and_clear();

    let loaded = loaded.load(Ordering::Acquire);
    if loaded == 1 {
        info!("loaded 1 resource");
    } else if loaded > 0 {
        info!("loaded {loaded} resources");
    }

    Ok(())
}

async fn load_project_resources(
    ctx: &CohostContext,
    state: &Mutex<CurrentStateV1>,
) -> anyhow::Result<()> {
    let total = ctx.total_project_resources_count().await?;

    info!("checking project resource files");

    let progress = ProgressBar::new(total);
    progress.set_style(long_progress_style());

    let loaded = Arc::new(AtomicU64::new(0));

    let pages = total.next_multiple_of(RESOURCE_LOAD_BATCH_SIZE) / RESOURCE_LOAD_BATCH_SIZE;
    for i in 0..pages {
        progress.set_position(i * RESOURCE_LOAD_BATCH_SIZE);

        let items = ctx
            .get_project_resources(
                i as i64 * RESOURCE_LOAD_BATCH_SIZE as i64,
                RESOURCE_LOAD_BATCH_SIZE as i64,
            )
            .await?;

        par_load_resources(
            ctx,
            state,
            &loaded,
            &items,
            |(project, _)| format!("project {project}"),
            |(_, url)| url,
        )
        .await?;
    }

    progress.finish_and_clear();

    let loaded = loaded.load(Ordering::Acquire);
    if loaded == 1 {
        info!("loaded 1 resource");
    } else if loaded > 0 {
        info!("loaded {loaded} resources");
    }

    Ok(())
}

async fn load_comment_resources(
    ctx: &CohostContext,
    state: &Mutex<CurrentStateV1>,
) -> anyhow::Result<()> {
    let total = ctx.total_comment_resources_count().await?;

    info!("checking comment resource files");

    let progress = ProgressBar::new(total);
    progress.set_style(long_progress_style());

    let loaded = Arc::new(AtomicU64::new(0));

    let pages = total.next_multiple_of(RESOURCE_LOAD_BATCH_SIZE) / RESOURCE_LOAD_BATCH_SIZE;
    for i in 0..pages {
        progress.set_position(i * RESOURCE_LOAD_BATCH_SIZE);

        let items = ctx
            .get_comment_resources(
                i as i64 * RESOURCE_LOAD_BATCH_SIZE as i64,
                RESOURCE_LOAD_BATCH_SIZE as i64,
            )
            .await?;

        par_load_resources(
            ctx,
            state,
            &loaded,
            &items,
            |(comment, _)| format!("comment {comment}"),
            |(_, url)| url,
        )
        .await?;
    }

    progress.finish_and_clear();

    let loaded = loaded.load(Ordering::Acquire);
    if loaded == 1 {
        info!("loaded 1 resource");
    } else if loaded > 0 {
        info!("loaded {loaded} resources");
    }

    Ok(())
}

async fn migrate_resource_file_paths(ctx: &CohostContext) -> anyhow::Result<()> {
    let total_count = ctx.total_url_file_count().await?;

    let progress = ProgressBar::new(total_count);
    progress.set_style(long_progress_style());

    progress.set_message("checking resource file paths");

    let mut migrated = 0;
    let mut offset = 0;
    loop {
        progress.set_position(offset as u64);

        let url_files = ctx.get_url_files_batch(offset, 1000).await?;

        offset += 1000;
        if url_files.is_empty() {
            break;
        }

        for (url, path) in url_files {
            let Ok(url) = Url::parse(&url) else { continue };
            let Some(intended_path) = ctx.get_intended_resource_file_path(&url).await? else {
                continue;
            };

            let intended_path = intended_path.strip_prefix(&ctx.root_dir)?;

            if path != intended_path {
                trace!(
                    "migrating resource file {} -> {}",
                    path.display(),
                    intended_path.display()
                );

                let from_path = ctx.root_dir.join(path);
                let to_path = ctx.root_dir.join(intended_path);
                fs::rename(from_path, to_path)?;
                ctx.insert_url_file(&url, intended_path).await?;
                migrated += 1;
            }
        }

        progress.set_message(format!(
            "checking resource file paths (migrated: {migrated})"
        ));
    }

    progress.finish_and_clear();

    Ok(())
}

pub async fn download(config: Config, db: SqliteConnection) {
    let mut ctx = CohostContext::new(
        config.cookie,
        Duration::from_secs(config.request_timeout_secs.unwrap_or(120)),
        PathBuf::from(&config.root_dir),
        db,
    );
    ctx.do_not_fetch_domains = config.do_not_fetch_domains.into_iter().collect();

    let mut state = ok_or_quit(CurrentStateV1::load_state().context("loading state"));

    let login = ok_or_quit(login(&ctx).await.context("logging in"));

    if !state.has_follows.contains(&login.project_id) {
        ok_or_quit(
            load_follows(&ctx, &login, &mut state)
                .await
                .context("loading follows"),
        );
        let _ = state.store_state();
    }

    let state = Arc::new(Mutex::new(state));

    let state2 = Arc::clone(&state);
    tokio::spawn(async move {
        loop {
            {
                trace!("writing state");
                let state = state2.lock().await;
                if let Err(e) = state.store_state() {
                    error!("could not save downloader state: {e}");
                    info!("here it is just for you:\n{state:?}");
                }
            }

            sleep(Duration::from_secs(1)).await;
        }
    });

    if !state.lock().await.has_likes.contains(&login.project_id) && config.load_likes {
        ok_or_quit(
            load_likes(&ctx, &login, &state)
                .await
                .context("loading likes"),
        );
    }

    for handle in &config.load_profile_posts {
        let project = if !ok_or_quit(ctx.has_project_handle(handle).await) {
            let project = ok_or_quit(
                ctx.projects_by_handle(handle)
                    .await
                    .with_context(|| format!("loading data for @{handle}")),
            );
            ok_or_quit(ctx.insert_project(&project).await);
            project.project_id
        } else {
            ok_or_quit(ctx.project_for_handle(handle).await).id as u64
        };

        let has_all_posts = state
            .lock()
            .await
            .projects
            .entry(project)
            .or_default()
            .has_all_posts;

        let new_only = has_all_posts && config.load_new_posts;

        if !has_all_posts || new_only {
            ok_or_quit(load_profile_posts(&ctx, &state, &login, project, new_only).await);

            ok_or_quit(ctx.db.vacuum().await);
        }
    }

    if config.load_dashboard {
        let followed_by_any = ok_or_quit(ctx.followed_by_any().await);
        for project in followed_by_any {
            let handle = ok_or_quit(ctx.project(project).await).handle;
            if config.skip_follows.contains(&handle) {
                continue;
            }

            let has_all_posts = state
                .lock()
                .await
                .projects
                .entry(project)
                .or_default()
                .has_all_posts;

            let new_only = has_all_posts && config.load_new_posts;

            if !has_all_posts || new_only {
                ok_or_quit(load_profile_posts(&ctx, &state, &login, project, new_only).await);

                ok_or_quit(ctx.db.vacuum().await);
            }
        }
    }

    for tag in &config.load_tagged_posts {
        let has_all_posts = state
            .lock()
            .await
            .tagged_posts
            .get(tag)
            .map(|s| s.has_all_posts)
            .unwrap_or(false);

        if !has_all_posts {
            ok_or_quit(load_tagged_posts(&ctx, &state, &login, tag).await);

            ok_or_quit(ctx.db.vacuum().await);
        }
    }

    if config.load_comments {
        let posts = {
            let mut state = state.lock().await;
            ok_or_quit(posts_without_comments(&ctx, &mut state).await)
        };

        if !posts.is_empty() {
            info!("loading comments");
        }
        ok_or_quit(load_comments_for_posts(&ctx, &state, &login, posts).await);
    }

    if config.try_fix_transparent_shares {
        ok_or_quit(fix_bad_transparent_shares(&ctx, &state, &login).await);
    }

    ok_or_quit(migrate_resource_file_paths(&ctx).await);

    ok_or_quit(load_cohost_resources(&ctx, &state).await);

    if config.load_post_resources {
        ok_or_quit(load_post_resources(&ctx, &state).await);
    }
    if config.load_project_resources {
        ok_or_quit(load_project_resources(&ctx, &state).await);
    }
    if config.load_comment_resources {
        ok_or_quit(load_comment_resources(&ctx, &state).await);
    }

    ok_or_quit(state.lock().await.store_state());

    info!("Done");
}
