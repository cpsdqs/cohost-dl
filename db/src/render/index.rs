use crate::data::Database;
use crate::render::PageRenderer;
use tera::Context;

impl PageRenderer {
    pub async fn render_index_page(&self, db: &Database) -> anyhow::Result<String> {
        let handles = db.get_all_project_handles_with_posts().await?;
        let dashboard_handles = db.project_handles_with_follows().await?;
        let liked_handles = db.project_handles_who_liked_posts().await?;

        let mut template_ctx = Context::new();
        template_ctx.insert("projects", &handles);
        template_ctx.insert("projects_with_dashboards", &dashboard_handles);
        template_ctx.insert("projects_who_liked_posts", &liked_handles);

        let body = self.tera.render("index.html", &template_ctx)?;

        Ok(body)
    }
}
