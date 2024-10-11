use crate::data::Database;
use crate::render::PageRenderer;
use tera::Context;

impl PageRenderer {
    pub async fn render_index_page(&self, db: &Database) -> anyhow::Result<String> {
        let handles = db.get_all_project_handles_with_posts().await?;

        let mut template_ctx = Context::new();
        template_ctx.insert("projects", &handles);

        let body = self.tera.render("index.html", &template_ctx)?;

        Ok(body)
    }
}
