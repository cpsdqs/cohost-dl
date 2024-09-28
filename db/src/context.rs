use crate::dl::CurrentStateV1;
use anyhow::{anyhow, bail, Context};
use diesel::SqliteConnection;
use reqwest::{Client, IntoUrl, StatusCode, Url};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::NamedTempFile;
use thiserror::Error;
use tokio::sync::Mutex;

const USER_AGENT: &str = "cohost-dl/2.0";
const TOTAL_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_FILE_NAME_LENGTH_UTF8: usize = 250;

pub struct CohostContext {
    cookie: String,
    client: Client,
    root_dir: PathBuf,
    pub do_not_fetch_domains: HashSet<String>,
    pub(crate) db: Mutex<SqliteConnection>,
}

struct ResourceUrlProps {
    fetch: Url,
    file_path: PathBuf,
    can_fail: bool,
    skip_file_ext_check: bool,
}

#[derive(Debug, Error)]
pub enum GetError {
    #[error(transparent)]
    Url(reqwest::Error),
    #[error("{0} not found: {1}")]
    NotFound(Url, String),
    #[error("{0} {1}: {2}")]
    OtherStatus(Url, StatusCode, String),
    #[error("GET {0}: {1}")]
    Req(Url, reqwest::Error),
    #[error(transparent)]
    Other(anyhow::Error),
}

impl CohostContext {
    pub fn new(cookie: String, root_dir: PathBuf, db: Mutex<SqliteConnection>) -> Self {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(TOTAL_REQUEST_TIMEOUT)
            .build()
            .expect("failed to create client");

        CohostContext {
            cookie,
            client,
            root_dir,
            do_not_fetch_domains: Default::default(),
            db,
        }
    }

    pub async fn get_text(&self, url: impl IntoUrl) -> Result<String, GetError> {
        let url = url.into_url().map_err(GetError::Url)?;
        trace!("GET {url}");

        let mut req = self.client.get(url.clone());

        if url.domain() == Some("cohost.org") {
            req = req.header("cookie", &self.cookie);
        }

        let res = req
            .send()
            .await
            .map_err(|e| GetError::Req(url.clone(), e))?;

        let status = res.status();
        let text = res
            .text()
            .await
            .map_err(|e| GetError::Req(url.clone(), e))?;

        if status.is_success() {
            Ok(text)
        } else if status == StatusCode::NOT_FOUND {
            Err(GetError::NotFound(url, text))
        } else {
            Err(GetError::OtherStatus(url, status, text))
        }
    }

    pub async fn get_json<T>(&self, url: impl IntoUrl) -> Result<T, GetError>
    where
        T: for<'a> Deserialize<'a> + 'static,
    {
        let url = url.into_url().map_err(GetError::Url)?;
        let text = self.get_text(url.clone()).await?;

        Ok(serde_json::from_str(&text)
            .map_err(|e| {
                let line = e.line().saturating_sub(1);
                let col = e.column().saturating_sub(1);

                let mut line_start = 0;
                for _ in 0..line {
                    let next_line_break = text[line_start..]
                        .char_indices()
                        .find(|(_, c)| *c == '\n')
                        .map(|(i, _)| i);
                    if let Some(next_line_break) = next_line_break {
                        line_start = next_line_break + 1;
                    }
                }

                let slice_center = line_start + col;
                let mut slice_start = slice_center;
                let mut slice_end = slice_center;

                for i in 0..4 {
                    let maybe_start = slice_center - 300 + i;
                    if text.is_char_boundary(maybe_start) {
                        slice_start = maybe_start;
                        break;
                    }
                }
                for i in 0..4 {
                    let maybe_end = (slice_center + 300).min(text.len()) + i;
                    if text.is_char_boundary(maybe_end) {
                        slice_end = maybe_end;
                        break;
                    }
                }

                let e = anyhow::Error::from(e);
                e.context(format!("excerpt: {}", &text[slice_start..slice_end]))
            })
            .map_err(GetError::Other)?)
    }

    pub async fn trpc_query<A, T>(&self, query: &str, input: Option<A>) -> Result<T, GetError>
    where
        A: Serialize,
        T: for<'a> Deserialize<'a> + 'static,
    {
        let mut url = Url::parse(&format!("https://cohost.org/api/v1/trpc/{query}")).unwrap();

        if let Some(input) = input {
            url.query_pairs_mut().append_pair(
                "input",
                &serde_json::to_string(&input).map_err(|e| GetError::Other(e.into()))?,
            );
        }

        #[derive(Deserialize)]
        enum TrpcResult<T> {
            #[serde(rename = "result")]
            Result { data: T },
            #[serde(rename = "error")]
            Error {
                code: i64,
                data: serde_json::Value,
                message: String,
            },
        }

        let result = self.get_json::<TrpcResult<T>>(url).await?;
        match result {
            TrpcResult::Result { data } => Ok(data),
            TrpcResult::Error {
                code,
                data,
                message,
            } => Err(GetError::Other(anyhow!(
                "TRPC error {code} / {data:?}: {message}"
            ))),
        }
    }

    pub async fn get_file(&self, url: impl IntoUrl) -> anyhow::Result<reqwest::Response> {
        let url = url.into_url()?;
        trace!("GET {url}");

        let res = self.client.get(url.clone()).send().await?;

        if res.status().is_success() {
            Ok(res)
        } else {
            let err = res.text().await?;

            let mut err_slice_end = err.len().min(500);
            for i in 0..4 {
                if err.is_char_boundary(err_slice_end + i) {
                    err_slice_end = err_slice_end + i;
                    break;
                }
            }

            bail!("failed at GET {url}: {}", &err[..err_slice_end])
        }
    }

    fn props_for_resource_url(&self, url: &Url) -> anyhow::Result<Option<ResourceUrlProps>> {
        if url.domain() == Some("staging.cohostcdn.org")
            && url
                .path_segments()
                .unwrap()
                .next()
                .map_or(false, |seg| seg.chars().all(|c| c.is_ascii_alphabetic()))
        {
            let mut file_path = self.root_dir.clone();
            file_path.push("rc");
            for seg in url.path_segments().unwrap() {
                file_path.push(seg);
            }

            Ok(Some(ResourceUrlProps {
                fetch: Url::parse(&format!(
                    "https://staging.cohostcdn.org/{}",
                    urlencoding::decode(url.path())?
                ))?,
                file_path,
                can_fail: false,
                skip_file_ext_check: true,
            }))
        } else if url.domain() == Some("cohost.org") {
            let mut file_path = self.root_dir.clone();
            for seg in url.path_segments().unwrap() {
                if seg.is_empty() {
                    continue;
                }
                file_path.push(seg);
            }

            Ok(Some(ResourceUrlProps {
                fetch: url.clone(),
                file_path,
                can_fail: false,
                skip_file_ext_check: true,
            }))
        } else if url.scheme() == "https"
            && !url
                .domain()
                .map_or(true, |d| self.do_not_fetch_domains.contains(d))
        {
            let mut file_path = self.root_dir.clone();
            file_path.push("rc");
            file_path.push("external");
            file_path.push(url.host_str().unwrap_or_default());

            let mut additional_path = url.path().to_string();
            if let Some(query) = url.query() {
                additional_path.push('?');
                additional_path.push_str(query);
            }
            if let Some(frag) = url.fragment() {
                additional_path.push('#');
                additional_path.push_str(frag);
            }
            if additional_path.starts_with('/') {
                additional_path.remove(0);
            }
            if additional_path.ends_with('/') {
                additional_path.pop();
            }

            if additional_path.is_empty() {
                additional_path.push('_');
            }

            for seg in additional_path.split('/') {
                if seg.len() > MAX_FILE_NAME_LENGTH_UTF8 {
                    let mut buf = String::new();
                    for c in seg.chars() {
                        buf.push(c);
                        if buf.len() >= MAX_FILE_NAME_LENGTH_UTF8 {
                            file_path.push(&buf);
                            buf.clear();
                        }
                    }
                    if !buf.is_empty() {
                        file_path.push(buf);
                    }
                } else {
                    file_path.push(seg);
                }
            }

            Ok(Some(ResourceUrlProps {
                fetch: url.clone(),
                file_path,
                can_fail: true,
                skip_file_ext_check: false,
            }))
        } else {
            trace!("ignoring URL {url}");
            Ok(None)
        }
    }

    fn add_content_type_ext(file_path: PathBuf, content_type: &str, should_warn: bool) -> PathBuf {
        if let Some(ext) = resource_file_extension_for_content_type(content_type) {
            if let Some(file_name) = file_path.file_name() {
                let mut file_name = file_name.to_os_string();
                // JS implementation was *specifically* appending, so we're not using with_extension
                file_name.push(".");
                file_name.push(ext);

                let mut file_path = file_path;
                file_path.pop();
                file_path.push(file_name);
                file_path
            } else {
                if should_warn {
                    warn!(
                        "could not fix resource extension for {}: no file path?",
                        file_path.display()
                    );
                }
                file_path
            }
        } else {
            if should_warn {
                warn!(
                    "did not add missing file extension for {} because of unknown content type {:?}",
                    file_path.display(),
                    content_type
                );
            }
            file_path
        }
    }

    /// Loads a resource to a file.
    /// Returns file path (relative to out dir) or None if it shouldn't be loaded
    pub async fn load_resource_to_file(
        &self,
        url: &Url,
        state: &Mutex<CurrentStateV1>,
        loaded: Option<&mut bool>,
    ) -> anyhow::Result<Option<PathBuf>> {
        if state.lock().await.failed_urls.contains(&url.to_string()) {
            return Ok(None);
        }

        let Some(props) = self.props_for_resource_url(url)? else {
            return Ok(None);
        };

        let needs_file_extension = !props.skip_file_ext_check
            && does_resource_probably_need_a_file_extension(&props.file_path);

        let file_path_with_ext = if needs_file_extension {
            let content_type = self.get_res_content_type(&props.fetch).await?;
            Self::add_content_type_ext(
                props.file_path.clone(),
                &content_type.unwrap_or_default(),
                false,
            )
        } else {
            props.file_path.clone()
        };

        if fs::exists(&file_path_with_ext)? {
            let result_file_path = file_path_with_ext
                .strip_prefix(&self.root_dir)
                .context("getting relative path")?
                .to_path_buf();

            return Ok(Some(result_file_path));
        }

        let mut res = match self.get_file(url.clone()).await {
            Ok(file) => file,
            Err(e) => {
                if props.can_fail {
                    state.lock().await.failed_urls.push(url.to_string());
                }
                Err(e).with_context(|| format!("loading resource at {url}"))?
            }
        };

        let content_type = if let Some(content_type) = res
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok().map(|s| s.to_string()))
        {
            self.insert_res_content_type(&props.fetch, &content_type)
                .await?;
            content_type
        } else {
            self.insert_res_content_type(&props.fetch, "").await?;
            String::new()
        };

        let file_path_with_ext = if needs_file_extension {
            Self::add_content_type_ext(props.file_path, &content_type, true)
        } else {
            props.file_path
        };

        let file =
            NamedTempFile::with_prefix("cohost-dl-res-").context("creating temporary file")?;

        while let Some(chunk) = res.chunk().await? {
            file.as_file().write_all(&chunk)?;
        }

        let result_file_path = file_path_with_ext
            .strip_prefix(&self.root_dir)
            .context("getting relative path")?
            .to_path_buf();

        let mut file_path_dir = file_path_with_ext.clone();
        file_path_dir.pop();
        fs::create_dir_all(file_path_dir)?;

        file.persist(&file_path_with_ext)
            .with_context(|| format!("moving resource to {}", file_path_with_ext.display()))?;

        if let Some(loaded) = loaded {
            *loaded = true;
        }

        Ok(Some(result_file_path))
    }
}

const KNOWN_FILE_EXTENSIONS: &[(&str, &[&str])] = &[
    // image formats
    ("apng", &["image/apng"]),
    ("avif", &["image/avif"]),
    ("bmp", &["image/bmp"]),
    ("gif", &["image/gif"]),
    ("heic", &["image/heic"]),
    ("heif", &["image/heif"]),
    ("ico", &["image/x-icon"]),
    ("jpeg", &["image/jpeg"]),
    ("jpg", &["image/jpeg"]),
    ("jfif", &["image/jpeg"]),
    ("jxl", &["image/jxl"]),
    ("png", &["image/png"]),
    ("svg", &["image/svg+xml", "image/svg"]),
    ("tif", &["image/tiff"]),
    ("tiff", &["image/tiff"]),
    ("webp", &["image/webp"]),
    // av formats
    ("flac", &["audio/flac"]),
    ("ogg", &["audio/ogg", "video/ogg", "application/ogg"]),
    ("opus", &["audio/opus"]),
    ("mp3", &["audio/mpeg"]),
    ("mp4", &["audio/mp4", "video/mp4"]),
    ("m4a", &["audio/mp4", "video/mp4"]),
    (
        "wav",
        &["audio/wav", "audio/vnd.wave", "audio/wave", "audio/x-wav"],
    ),
    // other resources
    ("css", &["text/css"]),
    ("js", &["application/javascript", "text/javascript"]),
    ("mjs", &["application/javascript", "text/javascript"]),
    ("json", &["application/json", "text/json"]),
    ("map", &[]),
    ("woff", &["font/woff"]),
    ("woff2", &["font/woff2"]),
];

fn does_resource_probably_need_a_file_extension(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext = ext.to_ascii_lowercase();
        let Some(ext) = ext.to_str() else { return true };

        for (e, _) in KNOWN_FILE_EXTENSIONS {
            if *e == ext {
                return false;
            }
        }
        true
    } else {
        true
    }
}

fn resource_file_extension_for_content_type(content_type: &str) -> Option<&'static str> {
    let base_content_type = content_type.split(';').next()?;

    for (e, c) in KNOWN_FILE_EXTENSIONS {
        if c.contains(&base_content_type) {
            return Some(*e);
        }
    }

    None
}
