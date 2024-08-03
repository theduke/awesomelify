use axum::{extract::State, Json};

use crate::{
    server::{ApiError, Ctx},
    storage::{Item, Storage},
};

pub const PATH_API_IMPORT: &'static str = "/api/v1/import";

#[derive(serde::Serialize)]
pub struct ImportResult {}

pub async fn handler_api_import(
    State(ctx): State<Ctx>,
    input: Json<Vec<Item>>,
) -> Result<Json<ImportResult>, ApiError> {
    ctx.store.import(input.0).await?;
    Ok(Json(ImportResult {}))
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;
    use time::OffsetDateTime;

    use crate::{
        server::{routes::api_export::PATH_API_EXPORT, test_client},
        source::{ReadmeRepo, RepoDetails, RepoIdent, RepoLink},
    };

    use super::*;

    #[tokio::test]
    async fn test_server_api_import_export() {
        let (client, _dir) = test_client().await;

        // Export should be empty.
        let res = client.get(PATH_API_EXPORT).send().await;

        let status = res.status();
        let body = res.text().await;
        dbg!(&body);

        assert_eq!(status.as_u16(), 200);
        let items = serde_json::from_str::<Vec<Item>>(&body).unwrap();
        assert_eq!(items, vec![]);

        let now = OffsetDateTime::now_utc();

        // Now import something.
        let items = vec![
            Item::Repo(crate::source::RepoDetailsItem::NotFound {
                ident: RepoIdent::parse_ident("github.com/org1/repo1").unwrap(),
                updated_at: now,
            }),
            Item::Repo(crate::source::RepoDetailsItem::Found(
                crate::source::RepoDetails {
                    ident: RepoIdent::parse_ident("github.com/org2/repo2").unwrap(),
                    description: Some("description".to_string()),
                    last_pushed_at: Some(now),
                    total_pull_requests: 33,
                    stargazer_count: 123,
                    fork_count: 44,
                    issues: 55,
                    last_pullrequest_merged_at: Some(now),
                    primary_language: Some("rust".to_string()),
                    languages: vec!["Rust".to_string(), "Typescript".to_string()],
                    updated_at: now,
                },
            )),
            Item::ReadmeRepo(ReadmeRepo {
                details: RepoDetails {
                    ident: RepoIdent::parse_ident("github.com/org3/awesome1").unwrap(),
                    description: Some("awesome desc".to_string()),
                    last_pushed_at: Some(now),
                    total_pull_requests: 99,
                    stargazer_count: 98,
                    fork_count: 97,
                    issues: 96,
                    last_pullrequest_merged_at: Some(now),
                    primary_language: Some("Markdown".to_string()),
                    languages: vec!["Markdown".to_string(), "text".to_string()],
                    updated_at: now,
                },
                readme_content: "readme!".to_string(),
                repo_links: vec![
                    RepoLink {
                        ident: RepoIdent::parse_ident("github.com/org1/repo1").unwrap(),
                        section: vec!["a".to_string(), "b".to_string()],
                    },
                    RepoLink {
                        ident: RepoIdent::parse_ident("github.com/org2/repo2").unwrap(),
                        section: vec!["b".to_string(), "c".to_string()],
                    },
                ],
                updated_at: now,
            }),
        ];

        let res = client.post(PATH_API_IMPORT).json(&items).send().await;
        assert_eq!(res.status(), StatusCode::OK);

        let items2 = client
            .get(PATH_API_EXPORT)
            .send()
            .await
            .json::<Vec<Item>>()
            .await;
        pretty_assertions::assert_eq!(items2, items);
    }
}
