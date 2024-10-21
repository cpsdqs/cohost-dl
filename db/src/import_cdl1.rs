use crate::context::{
    resource_file_extension_for_content_type, CohostContext, KNOWN_FILE_EXTENSIONS,
};
use crate::dl::{long_progress_style, CurrentStateV1};
use crate::project::ProjectFromCohost;
use crate::trpc::{ListEditedProjects, LoginLoggedIn, SinglePost};
use anyhow::{anyhow, Context};
use html5ever::tendril::TendrilSink;
use indicatif::ProgressBar;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::convert::identity;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::fs::{read_dir, read_to_string};
use tokio::sync::Mutex;

#[derive(Debug, Deserialize)]
pub struct CohostDl1ImportConfig {
    pub path: PathBuf,
    pub add_only: bool,
    pub reload: bool,
}

pub async fn import_cdl1(
    ctx: &CohostContext,
    state: &Mutex<CurrentStateV1>,
    config: CohostDl1ImportConfig,
) -> anyhow::Result<()> {
    let posts = {
        let mut posts = Vec::new();

        let mut read_projects = read_dir(&config.path)
            .await
            .context("reading the out directory")?;

        while let Some(entry) = read_projects
            .next_entry()
            .await
            .context("reading the out directory")?
        {
            let file_name = entry.file_name();
            let Some(handle) = file_name.to_str() else {
                continue;
            };
            if handle.starts_with('~') {
                continue;
            }
            if handle == "rc" || handle == "static" || handle == "api" {
                continue;
            }

            let project_post_dir = entry.path().join("post");
            if !project_post_dir.exists() {
                continue;
            }

            let mut read_posts = read_dir(&project_post_dir)
                .await
                .with_context(|| format!("reading {handle}/post/*"))?;

            while let Some(entry) = read_posts
                .next_entry()
                .await
                .with_context(|| format!("reading {handle}/post/*"))?
            {
                let file_name = entry.file_name();
                let Some(file_name) = file_name.to_str() else {
                    continue;
                };

                if file_name.ends_with(".html") {
                    // we only want data files
                    continue;
                }

                let meta = entry
                    .metadata()
                    .await
                    .with_context(|| format!("reading {handle}/post/{file_name} metadata"))?;

                if !meta.is_file()
                    || !file_name
                        .chars()
                        .next()
                        .map_or(false, |c| c.is_ascii_digit())
                {
                    continue;
                }

                posts.push(entry.path());
            }
        }

        posts.sort();

        posts
    };

    if posts.is_empty() {
        return Ok(());
    }

    if config.add_only {
        info!(
            "found {} post files to maybe import from cohost-dl 1",
            posts.len()
        );
    } else {
        info!(
            "found {} post files to import from cohost-dl 1",
            posts.len()
        );
    }

    let headers_file = config.path.join("~headers.json");
    if headers_file.exists() {
        import_headers(ctx, &headers_file)
            .await
            .context("importing data from ~headers.json")?;
    }

    let progress = ProgressBar::new(posts.len() as u64);
    progress.set_style(long_progress_style());

    progress.set_message("importing posts");

    let mut errors = Vec::new();
    let mut resources = HashSet::new();

    for file_path in posts {
        progress.inc(1);

        let maybe_stripped_file_path = file_path
            .strip_prefix(&config.path)
            .ok()
            .unwrap_or(&file_path);
        progress.set_message(format!("importing {}", maybe_stripped_file_path.display()));

        let mut post_resources = HashSet::new();
        let result = import_post_page(ctx, state, &file_path, &config, &mut post_resources).await;
        if let Err(e) = result {
            error!(
                "error importing {}: {e:?}\n",
                maybe_stripped_file_path.display()
            );
            errors.push(PathBuf::from(maybe_stripped_file_path));
        }

        let mut import_resources = HashSet::new();
        for res in post_resources {
            if resources.contains(&res) {
                continue;
            }
            import_resources.insert(res.clone());
            resources.insert(res);
        }
        copy_resources(ctx, &config.path, import_resources).await;
    }

    progress.finish_and_clear();

    if errors.is_empty() {
        info!("Finished importing cohost-dl 1 post data");
    } else {
        let error_count = errors.len();

        let error_paths = errors
            .into_iter()
            .map(|e| e.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        if error_count == 1 {
            info!("Finished importing cohost-dl 1 post data, with 1 failure:\n{error_paths}");
        } else {
            info!(
                "Finished importing cohost-dl 1 post data, with {} failures:\n{error_paths}",
                error_count
            );
        }
    }

    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SinglePostView {
    post_id: u64,
    project: ProjectFromCohost,
    #[serde(default)]
    nonce: Option<String>,
}

#[derive(Deserialize)]
struct SinglePostViewWrapper {
    #[serde(rename = "single-post-view")]
    single_post_view: SinglePostView,
}

#[derive(Deserialize)]
struct TrpcDehydratedState {
    queries: Vec<TrpcDehydratedQuery>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrpcDehydratedQuery {
    query_key: (TrpcQueryKeyId, TrpcQueryKeyData),
    state: TrpcDehydratedQueryState,
}

#[derive(Deserialize)]
struct TrpcDehydratedQueryState {
    data: Option<Value>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TrpcQueryKeyId {
    One(String),
    Multi(Vec<String>),
}

impl TrpcQueryKeyId {
    fn as_str(&self) -> String {
        match self {
            TrpcQueryKeyId::One(s) => s.clone(),
            TrpcQueryKeyId::Multi(parts) => parts.join("."),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TrpcQueryKeyData {
    #[allow(dead_code)]
    Str(String),
    Input {
        input: Option<Value>,
    },
}

impl TrpcDehydratedState {
    fn get(&self, query_id: &str, input: Option<Value>) -> Option<&Value> {
        self.queries
            .iter()
            .find(|q| {
                if &q.query_key.0.as_str() != query_id {
                    return false;
                }

                match &q.query_key.1 {
                    TrpcQueryKeyData::Input { input: i } => i == &input,
                    // we don't use str queries
                    _ => false,
                }
            })
            .and_then(|q| q.state.data.as_ref())
    }
}

async fn import_post_page(
    ctx: &CohostContext,
    state: &Mutex<CurrentStateV1>,
    file_path: &Path,
    config: &CohostDl1ImportConfig,
    resources: &mut HashSet<String>,
) -> anyhow::Result<()> {
    let html = read_to_string(&file_path)
        .await
        .with_context(|| format!("reading file {}", file_path.display()))?;

    let doc = kuchikiki::parse_html().one(html);

    let script = doc
        .select_first("script#__COHOST_LOADER_STATE__")
        .map_err(|()| anyhow!("could not find __COHOST_LOADER_STATE__ in post page"))?;
    let spv_data: SinglePostViewWrapper = serde_json::from_str(&script.text_contents())
        .context("parsing __COHOST_LOADER_STATE__ on post page")?;

    let script = doc
        .select_first("script#trpc-dehydrated-state")
        .map_err(|()| anyhow!("could not find trpc-dehydrated-state in post page"))?;
    let trpc_data: TrpcDehydratedState = serde_json::from_str(&script.text_contents())
        .context("parsing trpc-dehydrated-state on post page")?;

    let login = trpc_data
        .get("login.loggedIn", None)
        .ok_or(anyhow!("could not find query login.loggedIn in post page"))?;
    let login: LoginLoggedIn =
        serde_json::from_value(login.clone()).context("parsing login.loggedIn query")?;

    if !ctx.has_project_id(login.project_id).await? {
        let projects = trpc_data
            .get("projects.listEditedProjects", None)
            .ok_or(anyhow!(
                "could not find query projects.listEditedProjects in post page"
            ))?;
        let projects: ListEditedProjects = serde_json::from_value(projects.clone())
            .context("parsing projects.listEditedProjects")?;

        for project in projects.projects {
            ctx.insert_project(&project, config.add_only)
                .await
                .with_context(|| format!("inserting edited project @{}", project.handle))?;
        }
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct SinglePostInput {
        handle: String,
        post_id: u64,
    }

    let single_post_input = SinglePostInput {
        handle: spv_data.single_post_view.project.handle.clone(),
        post_id: spv_data.single_post_view.post_id,
    };

    let single_post_input = serde_json::to_value(single_post_input)?;

    let single_post = trpc_data
        .get("posts.singlePost", Some(single_post_input))
        .ok_or(anyhow!(
            "could not find query posts.singlePost in post page"
        ))?;
    let single_post: SinglePost =
        serde_json::from_value(single_post.clone()).context("parsing posts.singlePost")?;

    let already_has_post = ctx.has_post(single_post.post.post_id).await?;

    if config.add_only && already_has_post {
        // add any missing share posts
        for (i, post) in single_post.post.share_tree.iter().enumerate() {
            let prev_post = i.checked_sub(1).and_then(|i| post.share_tree.get(i));

            if !ctx.has_post(post.post_id).await? {
                debug!("adding missing share post {}", post.post_id);
                ctx.insert_post(ctx, state, &login, post, true, prev_post, true)
                    .await?;
            }
        }

        // only add any comments that might be new
        for comment in single_post.comments.values().flat_map(identity) {
            if !ctx.has_comment(&comment.comment.comment_id).await? {
                debug!("adding missing comment {}", comment.comment.comment_id);
                ctx.insert_comment(comment.comment.post_id, comment, true)
                    .await?;
            }
        }
    } else {
        ctx.insert_single_post(ctx, state, &login, &single_post, config.add_only)
            .await
            .context("inserting single post data")?;

        // add here so that we get resources even if reload fails
        add_all_resources_in_post(ctx, resources, spv_data.single_post_view.post_id).await?;

        if config.reload {
            let single_post = ctx
                .posts_single_post(
                    &spv_data.single_post_view.project.handle,
                    spv_data.single_post_view.post_id,
                    spv_data.single_post_view.nonce,
                )
                .await
                .context("reloading post from cohost.org (adding existing data succeeded!)")?;

            ctx.insert_single_post(ctx, state, &login, &single_post, config.add_only)
                .await
                .context("inserting updated single post data (adding existing data succeeded!)")?;
        }
    }

    add_all_resources_in_post(ctx, resources, spv_data.single_post_view.post_id).await?;

    Ok(())
}

async fn import_headers(ctx: &CohostContext, file_path: &Path) -> anyhow::Result<()> {
    #[derive(Deserialize)]
    struct Headers {
        #[serde(rename = "content-type")]
        content_type: String,
    }

    let headers = read_to_string(file_path).await.context("reading file")?;

    let headers: HashMap<String, Headers> =
        serde_json::from_str(&headers).context("parsing file")?;

    for (url, headers) in headers {
        if let Ok(url) = Url::parse(&url) {
            ctx.insert_res_content_type(&url, &headers.content_type)
                .await?;
        }
    }

    Ok(())
}

async fn add_all_resources_in_post(
    ctx: &CohostContext,
    resources: &mut HashSet<String>,
    post_id: u64,
) -> anyhow::Result<()> {
    let mut stack = vec![post_id];

    while let Some(post_id) = stack.pop() {
        for res in ctx.get_single_post_resources(post_id).await? {
            resources.insert(res);
        }

        let post = ctx.post(post_id).await?;
        for res in ctx
            .get_single_project_resources(post.posting_project_id as u64)
            .await?
        {
            resources.insert(res);
        }

        for comment in ctx.get_comments(post_id).await? {
            if let Some(project) = comment.posting_project_id {
                for res in ctx.get_single_project_resources(project as u64).await? {
                    resources.insert(res);
                }
            }

            for res in ctx.get_single_comment_resources(&comment.id).await? {
                resources.insert(res);
            }
        }

        if let Some(post) = post.share_of_post_id {
            stack.push(post as u64);
        }
    }

    Ok(())
}

fn where_would_cdl1_put_a_resource(url: &Url, content_type: Option<&str>) -> Option<String> {
    /// `r"^/[a-z]+/"`
    fn matches_re_a_z(s: &str) -> bool {
        let mut c = s.chars();
        if c.next() != Some('/') {
            return false;
        }
        let mut has_az = false;
        loop {
            match c.next() {
                Some(c) if c.is_ascii_alphabetic() => {
                    has_az = true;
                }
                Some('/') if has_az => break true,
                _ => break false,
            }
        }
    }

    fn decode_uri_component(s: &str) -> String {
        urlencoding::decode(s)
            .map(|s| s.to_string())
            .unwrap_or_else(|_| s.to_string())
    }

    fn split_too_long_file_name(s: String) -> String {
        const MAX_FILE_NAME_LENGTH_UTF8: usize = 250;

        let mut parts: Vec<_> = s.split('/').collect();

        while parts.last().map_or(false, |p| p.is_empty()) {
            // nodejs: trailing dir separators are ignored
            parts.pop();
        }

        let mut filename = parts.pop().unwrap_or_default();
        let mut dirname = parts.join("/");

        while filename.len() > MAX_FILE_NAME_LENGTH_UTF8 {
            let mut first_bit = String::new();
            let mut rest = filename;
            while first_bit.len() < MAX_FILE_NAME_LENGTH_UTF8 {
                let Some(item) = rest.chars().next() else {
                    break;
                };
                first_bit.push(item);
                rest = &rest[item.len_utf8()..];
            }

            dirname = format!("{dirname}/{first_bit}");
            filename = rest;
        }

        format!("{dirname}/{filename}")
    }

    fn get_clean_path(path: String) -> String {
        if cfg!(target_os = "windows") {
            path.chars()
                .map(|c| match c {
                    '?' | '%' | '*' | ':' | '|' | '"' | '<' | '>' => '-',
                    '/' => '\\', // Windows uses backslashes
                    c => c,
                })
                .collect()
        } else {
            path
        }
    }
    fn does_resource_file_path_probably_need_a_file_extension(file_path: &str) -> bool {
        let mut parts: Vec<_> = file_path.split('/').collect();

        while parts.last().map_or(false, |p| p.is_empty()) {
            // nodejs: trailing dir separators are ignored
            parts.pop();
        }

        let Some(file_name) = parts.pop() else {
            return true;
        };
        let Some(file_extension) = file_name.rsplit('.').next() else {
            return true;
        };
        let file_extension = file_extension.to_lowercase();

        KNOWN_FILE_EXTENSIONS
            .iter()
            .all(|(ext, _)| *ext != file_extension)
    }

    let file_path = if url.domain() == Some("staging.cohostcdn.org") && matches_re_a_z(url.path()) {
        get_clean_path(format!("rc{}", decode_uri_component(url.path())))
    } else if url.domain() == Some("cohost.org") {
        // no leading /
        let path = &url.path()[1..];
        get_clean_path(decode_uri_component(path))
    } else if url.scheme() == "https" {
        let Some(host) = url.host_str() else {
            return None;
        };
        let pathname = url.path();
        let search = url.query().map(|q| format!("?{q}")).unwrap_or_default();

        get_clean_path(split_too_long_file_name(format!(
            "rc/external/{host}{pathname}{search}"
        )))
    } else {
        return None;
    };

    if does_resource_file_path_probably_need_a_file_extension(&file_path) {
        if let Some(ext) = content_type.and_then(resource_file_extension_for_content_type) {
            return Some(format!("{file_path}.{ext}"));
        }
    }

    Some(file_path)
}

async fn copy_resources(ctx: &CohostContext, from_dir: &Path, resources: HashSet<String>) {
    for res in resources {
        if let Err(e) = maybe_copy_resource(ctx, from_dir, &res).await {
            error!("could not copy resource for {res}: {e:?}");
        }
    }
}

async fn maybe_copy_resource(
    ctx: &CohostContext,
    from_dir: &Path,
    url: &str,
) -> anyhow::Result<()> {
    let url = Url::parse(url).context("parsing URL")?;

    if ctx.get_url_file(&url).await?.is_some() {
        return Ok(());
    }

    let Some(target_path) = ctx.get_intended_resource_file_path(&url).await? else {
        return Ok(());
    };
    let Ok(target_path_stripped) = target_path.strip_prefix(&ctx.root_dir) else {
        anyhow::bail!(
            "not copying resource to {}: bad prefix",
            target_path.display()
        );
    };

    let content_type = ctx.get_res_content_type(&url).await?;

    let Some(path) = where_would_cdl1_put_a_resource(&url, content_type.as_deref()) else {
        return Ok(());
    };

    let path = from_dir.join(path);
    if path == target_path {
        return Ok(());
    }

    if !path.exists() {
        debug!("ignoring {}: not downloaded", path.display());
        return Ok(());
    }

    let mut target_dir = target_path.clone();
    target_dir.pop();
    fs::create_dir_all(target_dir)
        .await
        .context("creating directories")?;

    fs::copy(&path, &target_path)
        .await
        .context("copying resource")?;

    ctx.insert_url_file(&url, target_path_stripped).await?;

    Ok(())
}
