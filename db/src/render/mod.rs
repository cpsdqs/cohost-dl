use crate::render::md_render::MarkdownRenderer;
use tera::{Context, Tera};

pub mod api_data;
mod index;
pub mod md_render;
pub mod project_profile;
pub mod rewrite;
pub mod single_post;

pub struct PageRenderer {
    tera: Tera,
    md: MarkdownRenderer,
}

impl PageRenderer {
    pub fn new() -> Self {
        let mut tera = Tera::default();

        #[rustfmt::skip]
        let res = tera.add_raw_templates(vec![
            ("base.html", include_str!("../../templates/base.html")),
            ("comments.html", include_str!("../../templates/comments.html")),
            ("error.html", include_str!("../../templates/error.html")),
            ("index.html", include_str!("../../templates/index.html")),
            ("post.html", include_str!("../../templates/post.html")),
            ("project_profile.html", include_str!("../../templates/project_profile.html")),
            ("project_sidebar.html", include_str!("../../templates/project_sidebar.html")),
            ("single_post.html", include_str!("../../templates/single_post.html")),
        ]);

        if let Err(e) = res {
            eprintln!("{e}");
            std::process::exit(1);
        }

        let md = MarkdownRenderer::new(4);

        Self { tera, md }
    }

    pub fn render_error_page(&self, message: &str) -> String {
        let mut template_ctx = Context::new();
        template_ctx.insert("message", message);

        self.tera
            .render("error.html", &template_ctx)
            .unwrap_or("failed to render error page".into())
    }
}
