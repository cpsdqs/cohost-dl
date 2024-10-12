macro_rules! cdl_static {
    ($name:ident; $($item:literal,)+) => {
        pub const $name: &[(&str, &[u8])] = &[
            $(
            ($item, include_bytes!(concat!("../static/", $item))),
            )+
        ];
    };
}

cdl_static! {
    CDL_STATIC;
    "base.css",
    "base.js",
    "tailwind-prose.css",
}

/// these are hard-coded because they are very unlikely to change
pub const COHOST_STATIC: &str = include_str!("../cohost_static.txt");

pub const MD_RENDER_COMPILED: &str = include_str!("../md-render/compiled.js");

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
    "error.html",
    "index.html",
    "pagination_eggs.html",
    "post.html",
    "project_profile.html",
    "project_sidebar.html",
    "single_post.html",
    "tag_feed.html",
}
