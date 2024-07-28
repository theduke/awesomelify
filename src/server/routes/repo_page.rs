use axum::{
    extract::{Path, Query, State},
    response::Html,
};

use crate::{server::HtmlErrorPage, source::RepoIdent};

use crate::server::{ui, Ctx};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RepoPageView {
    SingleTable,
    TablePerCategory,
    List,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RepoSort {
    Title,
    Stars,
    Updated,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct RepoPageQuery {
    pub view: Option<RepoPageView>,
    pub sort: Option<RepoSort>,
}

impl RepoPageQuery {
    pub fn with_view(self, view: RepoPageView) -> Self {
        Self {
            view: Some(view),
            ..self
        }
    }

    pub fn with_sort(self, sort: RepoSort) -> Self {
        Self {
            sort: Some(sort),
            ..self
        }
    }

    pub fn to_query(&self) -> String {
        format!("?{}", serde_urlencoded::to_string(self).unwrap())
    }
}

pub async fn handler_repo(
    State(ctx): State<Ctx>,
    Path((source, owner, repo)): Path<(String, String, String)>,
    Query(query): Query<RepoPageQuery>,
) -> Result<Html<String>, HtmlErrorPage> {
    let ident = RepoIdent {
        source: source.parse()?,
        owner,
        repo,
    };
    let repo = ctx.loader.load_full_readme_repo(ident, true).await?;

    let html = ui::render_repo_page(repo.as_ref().clone(), query);

    Ok(Html(html))
}
