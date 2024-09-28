use crate::project::{AvatarShape, ProjectFlag, ProjectFromCohost, ProjectPrivacy};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostFromCohost {
    pub ast_map: PostAstMap,
    pub blocks: Vec<PostBlock>,
    pub can_publish: bool,
    pub can_share: bool,
    pub comments_locked: bool,
    pub contributor_block_incoming_or_outgoing: bool,
    pub cws: Vec<String>,
    pub effective_adult_content: bool,
    pub filename: String,
    pub has_any_contributor_muted: bool,
    pub has_cohost_plus: bool,
    /// No null value; will be empty string
    pub headline: String,
    pub is_editor: bool,
    pub is_liked: bool,
    pub limited_visibility_reason: LimitedVisibilityReason,
    pub num_comments: u64,
    pub num_shared_comments: u64,
    pub pinned: bool,
    pub plain_text_body: String,
    pub post_edit_url: String,
    pub post_id: u64,
    pub posting_project: ProjectFromCohost,
    /// ISO 8601
    pub published_at: Option<String>,
    pub related_projects: Vec<ProjectFromCohost>,
    pub response_to_ask_id: Option<String>,
    pub share_of_post_id: Option<u64>,
    pub share_tree: Vec<PostFromCohost>,
    pub shares_locked: bool,
    pub single_post_page_url: String,
    pub state: PostState,
    pub tags: Vec<String>,
    pub transparent_share_of_post_id: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostAstMap {
    pub read_more_index: Option<u64>,
    pub spans: Vec<PostAstMapSpan>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostAstMapSpan {
    start_index: u64,
    end_index: u64,
    // JSON string
    ast: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "kebab-case")]
pub enum PostBlock {
    Ask {
        ask: PostBlockAsk,
    },
    Attachment {
        attachment: PostBlockAttachment,
    },
    AttachmentRow {
        attachments: Vec<PostBlockAttachmentWrapper>,
    },
    Markdown {
        markdown: PostBlockMarkdown,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostBlockAsk {
    pub anon: bool,
    pub logged_in: bool,
    pub asking_project: Option<PostBlockAskProject>,
    pub ask_id: String,
    pub content: String,
    /// ISO 8601
    pub sent_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostBlockAskProject {
    pub project_id: u64,
    pub handle: String,
    #[serde(rename = "avatarURL")]
    pub avatar_url: String,
    #[serde(rename = "avatarPreviewURL")]
    pub avatar_preview_url: String,
    pub privacy: ProjectPrivacy,
    pub flags: Vec<ProjectFlag>,
    pub avatar_shape: AvatarShape,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostBlockMarkdown {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostBlockAttachmentWrapper {
    pub attachment: PostBlockAttachment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "kind")]
pub enum PostBlockAttachment {
    #[serde(rename_all = "camelCase")]
    Image {
        alt_text: Option<String>,
        attachment_id: Option<String>,
        #[serde(rename = "fileURL")]
        file_url: String,
        #[serde(rename = "previewURL")]
        preview_url: String,
        width: Option<u64>,
        height: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Audio {
        artist: Option<String>,
        title: Option<String>,
        #[serde(rename = "previewURL")]
        preview_url: String,
        #[serde(rename = "fileURL")]
        file_url: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostState {
    Draft = 0,
    Published = 1,
    Deleted = 2,
}

impl Serialize for PostState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            PostState::Draft => serializer.serialize_u32(0),
            PostState::Published => serializer.serialize_u32(1),
            PostState::Deleted => serializer.serialize_u32(2),
        }
    }
}

impl<'de> Deserialize<'de> for PostState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match u32::deserialize(deserializer)? {
            0 => Ok(Self::Draft),
            1 => Ok(Self::Published),
            2 => Ok(Self::Deleted),
            _ => Err(serde::de::Error::custom("invalid post state")),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LimitedVisibilityReason {
    None,
    LogInFirst,
    Deleted,
    Unpublished,
    AdultContent,
    Blocked,
}
