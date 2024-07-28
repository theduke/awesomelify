use anyhow::Context;

use super::{github::GithubClient, ReadmeRepo, RepoDetailsItem, RepoIdent, Source};

#[derive(Clone)]
pub struct SourceLoader {
    github: GithubClient,
}

impl SourceLoader {
    pub fn new(github: GithubClient) -> Self {
        Self { github }
    }

    pub async fn load_repo_details(
        &self,
        ident: &RepoIdent,
    ) -> Result<RepoDetailsItem, anyhow::Error> {
        tracing::trace!("loading repo details for {}", ident);
        let opt = match ident.source {
            Source::Github => self.github.repo_details(ident).await?,
        };

        if let Some(x) = opt {
            Ok(RepoDetailsItem::Found(x))
        } else {
            Ok(RepoDetailsItem::NotFound {
                ident: ident.clone(),
                updated_at: time::OffsetDateTime::now_utc(),
            })
        }
    }

    pub async fn load_readme_repo(&self, ident: &RepoIdent) -> Result<ReadmeRepo, anyhow::Error> {
        let (readme, details) = match ident.source {
            Source::Github => {
                tracing::trace!("loading README for {}", ident);
                let readme = self.github.repo_readme(ident).await?;
                let details = self
                    .github
                    .repo_details(ident)
                    .await?
                    .context("not found")?;
                (readme, details)
            }
        };

        let mut links = crate::markdown::parse_markdown(&readme)?;
        // Filter out links to self.
        links.retain(|link| link.ident != *ident);

        if links.is_empty() {
            anyhow::bail!("Does not appear to be an awesome- repo");
        }

        let repo = ReadmeRepo {
            details,
            readme_content: readme,
            repo_links: links,
            updated_at: time::OffsetDateTime::now_utc(),
        };

        Ok(repo)
    }
}
