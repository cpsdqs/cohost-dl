// @generated automatically by Diesel CLI.

diesel::table! {
    comment_resources (comment_id, url) {
        comment_id -> Text,
        url -> Text,
    }
}

diesel::table! {
    comments (id) {
        id -> Text,
        post_id -> Integer,
        in_reply_to_id -> Nullable<Text>,
        posting_project_id -> Nullable<Integer>,
        published_at -> Text,
        data -> Binary,
        data_version -> Integer,
    }
}

diesel::table! {
    follows (from_project_id, to_project_id) {
        from_project_id -> Integer,
        to_project_id -> Integer,
    }
}

diesel::table! {
    likes (from_project_id, to_post_id) {
        from_project_id -> Integer,
        to_post_id -> Integer,
    }
}

diesel::table! {
    post_related_projects (post_id, project_id) {
        post_id -> Integer,
        project_id -> Integer,
    }
}

diesel::table! {
    post_resources (post_id, url) {
        post_id -> Integer,
        url -> Text,
    }
}

diesel::table! {
    post_tags (post_id, tag) {
        post_id -> Integer,
        tag -> Text,
        pos -> Integer,
    }
}

diesel::table! {
    posts (id) {
        id -> Integer,
        posting_project_id -> Integer,
        published_at -> Nullable<Text>,
        response_to_ask_id -> Nullable<Text>,
        share_of_post_id -> Nullable<Integer>,
        is_transparent_share -> Bool,
        filename -> Text,
        data -> Binary,
        data_version -> Integer,
        state -> Integer,
    }
}

diesel::table! {
    project_resources (project_id, url) {
        project_id -> Integer,
        url -> Text,
    }
}

diesel::table! {
    projects (id) {
        id -> Integer,
        handle -> Text,
        is_private -> Bool,
        requires_logged_in -> Bool,
        data -> Binary,
        data_version -> Integer,
    }
}

diesel::table! {
    resource_content_types (url) {
        url -> Text,
        content_type -> Text,
    }
}

diesel::table! {
    url_files (url) {
        url -> Text,
        file_path -> Binary,
    }
}

diesel::joinable!(comment_resources -> comments (comment_id));
diesel::joinable!(comments -> posts (post_id));
diesel::joinable!(comments -> projects (posting_project_id));
diesel::joinable!(likes -> posts (to_post_id));
diesel::joinable!(likes -> projects (from_project_id));
diesel::joinable!(post_related_projects -> posts (post_id));
diesel::joinable!(post_related_projects -> projects (project_id));
diesel::joinable!(post_resources -> posts (post_id));
diesel::joinable!(post_tags -> posts (post_id));
diesel::joinable!(posts -> projects (posting_project_id));
diesel::joinable!(project_resources -> projects (project_id));

diesel::allow_tables_to_appear_in_same_query!(
    comment_resources,
    comments,
    follows,
    likes,
    post_related_projects,
    post_resources,
    post_tags,
    posts,
    project_resources,
    projects,
    resource_content_types,
    url_files,
);
