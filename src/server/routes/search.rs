use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

use crate::{
    server::{repo_page_uri, Ctx, HtmlError},
    source::RepoIdent,
};

pub const PATH_SEARCH: &str = "/search";

#[derive(serde::Deserialize, Debug, Clone)]
pub struct SearchQuery {
    pub q: String,
}

pub async fn handler_search(
    State(ctx): State<Ctx>,
    Query(q): Query<SearchQuery>,
) -> Result<Response, HtmlError> {
    match search(&ctx, &q).await {
        Ok(res) => Ok(res),
        Err(err) => {
            let mut res = err.into_response();
            // HTMX needs a successful status code to show the response.
            *res.status_mut() = StatusCode::OK;
            Ok(res)
        }
    }
}

async fn search(ctx: &Ctx, query: &SearchQuery) -> Result<Response, HtmlError> {
    // Parse the query.

    let ident = match RepoIdent::parse_ident(query.q.trim()) {
        Ok(url) => url,
        Err(err) => {
            return Err(HtmlError::msg(
                format!("Invvalid url '{}': {}", query.q, err),
                StatusCode::BAD_REQUEST,
            ));
        }
    };

    let readme = ctx.loader.load_full_readme_repo(ident, true).await?;

    if readme.repo.repo_links.is_empty() {
        return Err(HtmlError::msg(
            "No repositories found in readme - is this a aweomse list?",
            StatusCode::NOT_FOUND,
        ));
    }

    let mut res = Html("Repo loaded!".to_string()).into_response();
    res.headers_mut().append(
        "HX-Redirect",
        repo_page_uri(&readme.repo.details.ident).parse().unwrap(),
    );

    Ok(res)
}
