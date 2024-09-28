use crate::context::CohostContext;
use crate::post::PostFromCohost;
use anyhow::{anyhow, Context};
use html5ever::tendril::TendrilSink;
use reqwest::Url;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginationMode {
    pub current_skip: u64,
    pub ideal_page_stride: u64,
    pub mode: String,
    pub more_pages_backward: bool,
    pub more_pages_forward: bool,
    pub page_url_factory_name: String,
    pub ref_timestamp: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostsFeed {
    pub highlighted_tags: Vec<String>,
    pub no_posts_string_id: String,
    pub pagination_mode: PaginationMode,
    pub posts: Vec<PostFromCohost>,
}

#[derive(Debug, Deserialize)]
struct LikedPostsFeed {
    #[serde(rename = "liked-posts-feed")]
    liked_posts_feed: PostsFeed,
}

impl CohostContext {
    pub async fn load_liked_posts(
        &self,
        ref_timestamp: Option<u64>,
        skip_posts: u64,
    ) -> anyhow::Result<PostsFeed> {
        let mut url = Url::parse("https://cohost.org/rc/liked-posts")?;
        if let Some(ref_timestamp) = ref_timestamp {
            url.query_pairs_mut()
                .append_pair("refTimestamp", &ref_timestamp.to_string());
        }
        if skip_posts > 0 {
            url.query_pairs_mut()
                .append_pair("skipPosts", &skip_posts.to_string());
        }

        let html = self
            .get_text(url)
            .await
            .context("loading liked posts page")?;

        let doc = kuchikiki::parse_html().one(html);
        let script = doc
            .select_first("script#__COHOST_LOADER_STATE__")
            .map_err(|()| anyhow!("could not find __COHOST_LOADER_STATE__ in liked posts page"))?;

        let data: LikedPostsFeed = serde_json::from_str(&script.text_contents())
            .context("parsing __COHOST_LOADER_STATE__ on liked posts page")?;

        Ok(data.liked_posts_feed)
    }
}
