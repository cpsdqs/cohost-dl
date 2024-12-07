use crate::comment::Permission;
use crate::context::CohostContext;
use crate::post::PostFromCohost;
use anyhow::{anyhow, Context};
use html5ever::tendril::TendrilSink;
use reqwest::Url;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(unused)]
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
    #[allow(unused)]
    pub highlighted_tags: Vec<String>,
    #[allow(unused)]
    pub no_posts_string_id: String,
    pub pagination_mode: PaginationMode,
    pub posts: Vec<PostFromCohost>,
}

#[derive(Debug, Deserialize)]
struct LikedPostsFeed {
    #[serde(rename = "liked-posts-feed")]
    liked_posts_feed: PostsFeed,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaggedPostsFeed {
    #[allow(unused)]
    pub no_posts_string_id: String,
    pub pagination_mode: PaginationMode,
    pub posts: Vec<PostFromCohost>,
    pub synonyms_and_related_tags: Vec<RelatedTag>,
    #[allow(unused)]
    pub tag_name: String,
    #[allow(unused)]
    pub show_18_plus_posts: bool,
}

#[derive(Debug, Deserialize)]
pub struct RelatedTag {
    #[allow(unused)]
    pub tag_id: String,
    pub content: String,
    pub relationship: TagRelationship,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TagRelationship {
    Related,
    Synonym,
}

#[derive(Debug, Deserialize)]
struct TaggedPostFeedContainer {
    #[serde(rename = "tagged-post-feed")]
    tagged_post_feed: TaggedPostsFeed,
}

// Not a feed, but this is the file where all the others of this type are
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPageView {
    pub can_access_permissions: ProjectCanAccessPermissions,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCanAccessPermissions {
    pub can_read: Permission,
    pub can_interact: Permission,
    pub can_share: Permission,
    pub can_edit: Permission,
}

#[derive(Debug, Deserialize)]
struct ProjectPageViewContainer {
    #[serde(rename = "project-page-view")]
    project_page_view: ProjectPageView,
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

    pub async fn load_tagged_posts(
        &self,
        tag: &str,
        ref_timestamp: Option<u64>,
        skip_posts: u64,
    ) -> anyhow::Result<TaggedPostsFeed> {
        let tag_encoded = urlencoding::encode(tag);
        let mut url = Url::parse(&format!("https://cohost.org/rc/tagged/{tag_encoded}"))?;

        url.query_pairs_mut().append_pair("show18PlusPosts", "true");

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
            .context("loading tagged posts page")?;

        let doc = kuchikiki::parse_html().one(html);
        let script = doc
            .select_first("script#__COHOST_LOADER_STATE__")
            .map_err(|()| anyhow!("could not find __COHOST_LOADER_STATE__ in tagged posts page"))?;

        let data: TaggedPostFeedContainer = serde_json::from_str(&script.text_contents())
            .context("parsing __COHOST_LOADER_STATE__ on tagged posts page")?;

        Ok(data.tagged_post_feed)
    }

    pub async fn project_page_view(&self, handle: &str) -> anyhow::Result<ProjectPageView> {
        let url = Url::parse(&format!("https://cohost.org/{handle}"))?;

        let html = self
            .get_text(url)
            .await
            .context("loading project page view")?;

        let doc = kuchikiki::parse_html().one(html);
        let script = doc
            .select_first("script#__COHOST_LOADER_STATE__")
            .map_err(|()| anyhow!("could not find __COHOST_LOADER_STATE__ in project page view"))?;

        let data: ProjectPageViewContainer = serde_json::from_str(&script.text_contents())
            .context("parsing __COHOST_LOADER_STATE__ on project page view")?;

        Ok(data.project_page_view)
    }
}
