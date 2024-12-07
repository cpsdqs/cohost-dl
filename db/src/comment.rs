use crate::project::ProjectFromCohost;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentFromCohost {
    pub poster: Option<ProjectFromCohost>,
    pub comment: InnerComment,
    pub can_edit: Permission,
    pub can_hide: Permission,
    pub can_interact: Permission,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InnerComment {
    pub body: String,
    pub comment_id: String,
    pub children: Vec<CommentFromCohost>,
    pub deleted: bool,
    pub has_cohost_plus: bool,
    pub hidden: bool,
    pub in_reply_to: Option<String>,
    pub post_id: u64,
    #[serde(rename = "postedAtISO")]
    pub posted_at_iso: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Permission {
    Allowed,
    NotAllowed,
    LogInFirst,
    Blocked,
}
