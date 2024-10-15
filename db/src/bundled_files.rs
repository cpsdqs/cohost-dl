macro_rules! cdl_static {
    ($name:ident; $($item_name:literal: $item_src:literal,)+) => {
        pub const $name: &[(&str, &[u8])] = &[
            $(
            ($item_name, include_bytes!(concat!("../", $item_src))),
            )+
        ];
    };
}

cdl_static! {
    CDL_STATIC;
    "base.css": "static/base.css",
    "tailwind-prose.css": "static/tailwind-prose.css",
    "client.js": "md-render/dist/client.js",
}

/// these are hard-coded because they are very unlikely to change
pub const COHOST_STATIC: &str = include_str!("../cohost_static.txt");

pub const MD_RENDER_COMPILED: &str = include_str!("../md-render/dist/server-render.js");

pub const TEMPLATE_CONFIG: &str = include_str!("../config.example.toml");

macro_rules! templates {
    ($name:ident; $($item:literal,)+) => {
        pub const $name: &[(&str, &str)] = &[
            $(
            ($item, include_str!(concat!("../templates/", $item))),
            )+
        ];
    };
}

templates! {
    TEMPLATES;
    "base.html",
    "comments.html",
    "dashboard.html",
    "error.html",
    "index.html",
    "liked_feed.html",
    "pagination_eggs.html",
    "post.html",
    "project_profile.html",
    "project_sidebar.html",
    "single_post.html",
    "tag_feed.html",
}
