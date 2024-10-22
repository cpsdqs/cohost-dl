use crate::bundled_files::TEMPLATES;
use crate::render::md_render::MarkdownRenderer;
use tera::{Context, Tera};

pub mod api_data;
pub mod feed;
pub mod index;
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
        let res = tera.add_raw_templates(TEMPLATES.iter().copied());

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
