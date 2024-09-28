use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFromCohost {
    pub ask_settings: ProjectAskSettings,
    #[serde(rename = "avatarPreviewURL")]
    pub avatar_preview_url: String,
    pub avatar_shape: AvatarShape,
    #[serde(rename = "avatarURL")]
    pub avatar_url: String,
    pub contact_card: Vec<ProjectContactCard>,
    pub dek: String,
    pub delete_after: Option<String>,
    pub description: String,
    pub display_name: String,
    pub flags: Vec<ProjectFlag>,
    pub frequently_used_tags: Vec<String>,
    pub handle: String,
    #[serde(rename = "headerPreviewURL")]
    pub header_preview_url: Option<String>,
    #[serde(rename = "headerURL")]
    pub header_url: Option<String>,
    pub is_self_project: Option<bool>,
    pub logged_out_post_visibility: LoggedOutPostVisibility,
    pub privacy: ProjectPrivacy,
    pub project_id: u64,
    pub pronouns: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AvatarShape {
    Circle,
    Roundrect,
    Squircle,
    CapsuleBig,
    CapsuleSmall,
    Egg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectPrivacy {
    Public,
    Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LoggedOutPostVisibility {
    Public,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectFlag {
    Staff,
    StaffMember,
    FriendOfTheSite,
    NoTransparentAvatar,
    Suspended,
    Automated,
    Parody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAskSettings {
    enabled: bool,
    allow_anon: bool,
    require_logged_in_anon: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectContactCard {
    service: String,
    value: String,
    visibility: ContactCardVisibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContactCardVisibility {
    Public,
    LoggedIn,
    Follows,
    FollowingYou,
}
