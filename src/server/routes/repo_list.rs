use axum::{extract::State, response::Html};

use crate::server::{ui, Ctx, HtmlErrorPage};

pub const PATH_README_LIST: &str = "/lists";

pub async fn handler_readme_list(State(ctx): State<Ctx>) -> Result<Html<String>, HtmlErrorPage> {
    let repos = ctx.loader.popular_repos(200).await?;

    let html = ui::render_readme_list_page(repos);

    Ok(Html(html))
}
