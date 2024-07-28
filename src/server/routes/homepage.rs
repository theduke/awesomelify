use axum::{extract::State, response::Html};

use crate::server::{ui, Ctx, HtmlErrorPage};

pub async fn handler_homepage(State(ctx): State<Ctx>) -> Result<Html<String>, HtmlErrorPage> {
    let repos = ctx.loader.popular_repos(12).await?;

    let html = ui::render_homepage(repos);

    Ok(Html(html))
}
