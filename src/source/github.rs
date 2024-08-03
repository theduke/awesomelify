use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use anyhow::Context;
use base64::Engine;
use query_repo_details::RepoDetailsResponse;
use reqwest::RequestBuilder;

use crate::source::RepoDetails;

use super::{RateLimitError, RepoIdent};

#[derive(Clone)]
pub struct GithubClient {
    client: reqwest::Client,
    rate_limited_until: Arc<Mutex<Option<SystemTime>>>,
}

impl GithubClient {
    pub fn from_env() -> Self {
        let token = std::env::var("GITHUB_TOKEN").ok();
        Self::new(token)
    }

    pub fn new(token: Option<String>) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(token) = token {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", token)
                    .parse()
                    .expect("Invalid Github auth token"),
            );
        }

        let client = reqwest::Client::builder()
            .user_agent("awesomelify")
            .connect_timeout(Duration::from_secs(10))
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();
        GithubClient {
            client,
            rate_limited_until: Arc::new(Mutex::new(None)),
        }
    }

    pub fn rate_limited_until(&self) -> Option<SystemTime> {
        let mut lock = self.rate_limited_until.lock().unwrap();

        if let Some(target) = *lock {
            let now = SystemTime::now();
            if target >= now {
                return Some(target);
            } else {
                // Time expired. Reset to avoid redundant work.
                *lock = None;
            }
        }

        None
    }

    fn set_rate_limited_until(&self, until: SystemTime) {
        if until >= SystemTime::now() {
            let mut lock = self.rate_limited_until.lock().unwrap();
            *lock = Some(until);
        }
    }

    async fn fetch(&self, builder: RequestBuilder) -> Result<reqwest::Response, anyhow::Error> {
        if let Some(until) = self.rate_limited_until() {
            return Err(RateLimitError {
                message: "Github API rate limit exceeded".to_string(),
                reset_at: Some(until),
            }
            .into());
        }

        let res = builder.send().await?;
        let status = res.status();
        if !status.is_success() && (status == 403 || status == 429) {
            let reset_at = res
                .headers()
                .get("x-ratelimit-reset")
                .and_then(|x| x.to_str().ok())
                .and_then(|x| x.parse::<u64>().ok());

            if let Some(reset) = reset_at {
                let reset_at = SystemTime::UNIX_EPOCH + Duration::from_secs(reset);
                self.set_rate_limited_until(reset_at);

                return Err(RateLimitError {
                    message: "Github API rate limit exceeded".to_string(),
                    reset_at: Some(reset_at),
                }
                .into());
            }
        }
        Ok(res)
    }

    async fn graphql<V, D>(
        &self,
        query: impl Into<String>,
        variables: V,
    ) -> Result<D, anyhow::Error>
    where
        V: serde::Serialize,
        D: serde::de::DeserializeOwned,
    {
        let query = GraphqlQuery {
            query: query.into(),
            variables,
        };

        let req = self
            .client
            .post("https://api.github.com/graphql")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::ACCEPT, "application/json")
            .json(&query);

        let res = self.fetch(req).await?;

        if !res.status().is_success() {
            let error = res.text().await?;
            anyhow::bail!("GraphQL request failed: {}", error);
        }

        let body = res.text().await?;
        let data: GraphqlResponse<D> = match deserialize_json(&body) {
            Ok(v) => v,
            Err(err) => Err(err).context("failed to parse json response")?,
        };

        data.data.context("GraphQL response returned no data")
    }

    pub async fn repo_readme(&self, ident: &RepoIdent) -> Result<String, anyhow::Error> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/readme",
            ident.owner, ident.repo
        );
        let req = self.client.get(&url);
        let res = self
            .fetch(req)
            .await?
            .error_for_status()?
            .json::<ReadmeData>()
            .await?;

        if res.encoding != "base64" {
            anyhow::bail!("unexpected encoding: {}", res.encoding);
        }

        let content = base64::engine::general_purpose::STANDARD
            .decode(res.content.replace("\n", ""))
            .context("failed to decode README base64")?;

        let content = String::from_utf8(content).context("non-UTF8 readme")?;

        Ok(content)
    }

    pub async fn repo_details(
        &self,
        ident: &RepoIdent,
    ) -> Result<Option<RepoDetails>, anyhow::Error> {
        let res = self
            .graphql::<_, RepoDetailsResponse>(
                query_repo_details::REPO_DETAILS_QUERY,
                RepoVariables {
                    owner: ident.owner.clone(),
                    repo: ident.repo.clone(),
                },
            )
            .await;

        let data = match res {
            Ok(v) => v,
            Err(err) => {
                if err
                    .to_string()
                    .to_lowercase()
                    .contains("could not resolve to a repository")
                {
                    return Ok(None);
                } else {
                    return Err(err);
                }
            }
        };

        let Some(repo) = data.repository else {
            return Ok(None);
        };

        let data = RepoDetails {
            ident: ident.clone(),
            description: repo.description,
            total_pull_requests: repo.total_pull_requests.total_count,
            stargazer_count: repo.stargazer_count,
            fork_count: repo.fork_count,
            issues: repo.issues.total_count,
            last_pushed_at: repo.pushed_at,
            last_pullrequest_merged_at: repo
                .latest_merged_pull_request
                .nodes
                .first()
                .map(|x| x.merged_at),
            primary_language: repo.primary_language.map(|x| x.name),
            languages: repo
                .languages
                .nodes
                .iter()
                .map(|x| x.name.clone())
                .collect(),
            updated_at: time::OffsetDateTime::now_utc(),
        };

        Ok(Some(data))
    }
}

#[derive(serde::Deserialize, Debug)]
struct ReadmeData {
    content: String,
    encoding: String,
}

#[derive(serde::Serialize, Debug)]
struct GraphqlQuery<V> {
    query: String,
    variables: V,
}

#[derive(serde::Deserialize, Debug)]
struct GraphqlResponse<V> {
    data: Option<V>,
    #[allow(dead_code)]
    errors: Option<Vec<Error>>,
}

#[derive(serde::Deserialize, Debug)]
struct Error {
    #[allow(dead_code)]
    message: String,
}

#[derive(serde::Serialize, Debug)]
struct RepoVariables {
    owner: String,
    repo: String,
}

mod query_repo_details {
    use serde::Deserialize;
    use time::OffsetDateTime;

    pub const REPO_DETAILS_QUERY: &str = r#"
query ($owner: String!, $repo: String!) {
  repository(owner: $owner, name: $repo) {
    owner {
      login
    }
    name
    stargazerCount
    forkCount
    description
    pushedAt
    totalPullRequests: pullRequests {
      totalCount
    }
    issues {
      totalCount
    }
    latestMergedPullRequest: pullRequests(
      orderBy: {field: UPDATED_AT, direction: DESC}
      first: 1
      states: MERGED
    ) {
      nodes {
        mergedAt
      }
    }
    primaryLanguage {
      name
      color
    }
    languages(first:3, orderBy:{
      field:SIZE,
      direction:DESC
      
    }) {
      nodes {
        name
        color
      }
    }
  }
}
"#;

    #[derive(Deserialize, Debug)]
    pub struct RepoDetailsResponse {
        pub repository: Option<Repository>,
    }

    #[derive(Deserialize, Debug)]
    pub struct Repository {
        // pub owner: Owner,
        // pub name: String,
        #[serde(rename = "stargazerCount")]
        pub stargazer_count: u32,
        #[serde(rename = "forkCount")]
        pub fork_count: u32,
        #[serde(rename = "pushedAt", with = "time::serde::iso8601::option")]
        pub pushed_at: Option<OffsetDateTime>,
        pub description: Option<String>,
        #[serde(rename = "totalPullRequests")]
        pub total_pull_requests: TotalCount,
        #[serde(rename = "latestMergedPullRequest")]
        pub latest_merged_pull_request: Connection<SparsePullRequest>,
        pub issues: TotalCount,
        #[serde(rename = "primaryLanguage")]
        pub primary_language: Option<Language>,
        pub languages: Connection<Language>,
    }

    #[derive(Deserialize, Debug, PartialEq, Eq)]
    pub struct Language {
        pub name: String,
        pub color: Option<String>,
    }

    #[derive(Deserialize, Debug)]
    pub struct Connection<T> {
        pub nodes: Vec<T>,
    }

    #[derive(Deserialize, Debug)]
    pub struct TotalCount {
        #[serde(rename = "totalCount")]
        pub total_count: u32,
    }

    #[derive(Deserialize, Debug)]
    pub struct SparsePullRequest {
        #[serde(rename = "mergedAt", with = "time::serde::iso8601")]
        pub merged_at: OffsetDateTime,
    }

    #[derive(Deserialize, Debug)]
    pub struct Owner {
        // pub login: String,
    }
}

fn deserialize_json<T>(raw: &str) -> Result<T, serde_path_to_error::Error<serde_json::Error>>
where
    T: serde::de::DeserializeOwned,
{
    let jd = &mut serde_json::Deserializer::from_str(raw);
    serde_path_to_error::deserialize(jd)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_client() -> Option<GithubClient> {
        let token = std::env::var("GITHUB_TOKEN").ok()?;
        Some(GithubClient::new(Some(token)))
    }

    macro_rules! test_client {
        () => {
            if let Some(client) = test_client() {
                client
            } else {
                eprintln!("Skipping Github API test: GITHUB_TOKEN not set!");
                return;
            }
        };
    }

    #[tokio::test]
    async fn test_github_client_readme() {
        let client = test_client!();

        let id = RepoIdent::new_github("rust-unofficial", "awesome-rust");
        let readme = client.repo_readme(&id).await.unwrap();

        assert!(readme.contains("Awesome Rust"));
    }

    #[tokio::test]
    async fn test_github_repo_details() {
        let client = test_client!();

        // let id = RepoIdent::new_github("theduke", "easyduration");
        let id = RepoIdent::new_github("rust-unofficial", "awesome-rust");
        let data = client.repo_details(&id).await.unwrap().unwrap();
        assert!(data.ident == id);
        assert!(data.stargazer_count >= 2);
        assert!(data.fork_count > 0);
        assert_eq!(data.primary_language.as_deref(), Some("Rust"),);
    }
}
