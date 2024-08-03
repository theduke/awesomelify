use std::time::SystemTime;

use anyhow::{anyhow, bail, Context};
use time::OffsetDateTime;

pub mod github;
pub mod loader;

#[derive(
    serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum Source {
    Github,
}

impl Source {
    const fn as_str(&self) -> &'static str {
        match self {
            Source::Github => "github",
        }
    }

    const fn domain(&self) -> &'static str {
        match self {
            Source::Github => "github.com",
        }
    }
}

impl std::str::FromStr for Source {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "github" => Ok(Source::Github),
            _ => bail!("unknown source: {}", s),
        }
    }
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct RepoIdent {
    pub source: Source,
    pub owner: String,
    pub repo: String,
}

impl Ord for RepoIdent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.source
            .cmp(&other.source)
            .then_with(|| self.owner.cmp(&other.owner))
            .then_with(|| self.repo.cmp(&other.repo))
    }
}

impl PartialOrd for RepoIdent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl RepoIdent {
    pub fn new(source: Source, owner: impl Into<String>, repo: impl Into<String>) -> Self {
        Self {
            source,
            owner: owner.into(),
            repo: repo.into(),
        }
    }

    pub fn new_github(owner: impl Into<String>, repo: impl Into<String>) -> Self {
        Self::new(Source::Github, owner, repo)
    }

    pub fn parse_url(url: &str) -> Result<Self, anyhow::Error> {
        let url: url::Url = url.parse()?;

        match url.host_str() {
            Some("github.com") => {
                let mut path = url.path().split('/').skip(1);
                let owner = path
                    .next()
                    .map(|x| x.trim())
                    .filter(|x| !x.is_empty())
                    .ok_or_else(|| anyhow!("missing owner"))?;
                let repo = path
                    .next()
                    .map(|x| x.trim())
                    .filter(|x| !x.is_empty())
                    .ok_or_else(|| anyhow!("missing repo"))?;

                Ok(Self::new_github(owner, repo))
            }
            Some(host) => bail!("unsupported host: {}", host),
            None => bail!("missing host"),
        }
    }

    pub fn parse_ident(ident: &str) -> Result<Self, anyhow::Error> {
        if let Ok(url) = Self::parse_url(ident) {
            return Ok(url);
        }

        if ident.starts_with("github.com/") {
            let rest = ident.trim_start_matches("github.com/");
            let (org, repo) = rest
                .split_once('/')
                .filter(|(owner, repo)| {
                    !owner.is_empty() && !repo.is_empty() && !repo.contains('/')
                })
                .context("invalid github.com/ URL - expected github.com/<org>/<repo>")?;

            return Ok(Self::new_github(org, repo));
        }

        let (org, repo) = ident
            .split_once('/')
            .filter(|(owner, repo)| !owner.is_empty() && !repo.is_empty() && !repo.contains('/'))
            .context("invalid Github repo - expected <org>/<repo>")?;

        Ok(Self::new_github(org, repo))
    }

    pub fn url(&self) -> String {
        let domain = self.source.domain();
        format!("https://{}/{}/{}", domain, self.owner, self.repo)
    }

    pub fn pretty_url(&self) -> String {
        format!("{}/{}/{}", self.source.domain(), self.owner, self.repo)
    }

    pub fn name(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }
}

impl std::fmt::Display for RepoIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}/{}", self.source, self.owner, self.repo)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ReadmeRepo {
    pub details: RepoDetails,
    pub readme_content: String,
    pub repo_links: Vec<RepoLink>,
    pub updated_at: time::OffsetDateTime,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RepoLink {
    pub ident: RepoIdent,
    pub section: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RepoDetails {
    pub ident: RepoIdent,

    pub description: Option<String>,

    #[serde(default, with = "time::serde::iso8601::option")]
    pub last_pushed_at: Option<OffsetDateTime>,
    pub total_pull_requests: u32,
    pub stargazer_count: u32,
    pub fork_count: u32,
    pub issues: u32,

    #[serde(default, with = "time::serde::iso8601::option")]
    pub last_pullrequest_merged_at: Option<OffsetDateTime>,
    pub primary_language: Option<String>,
    pub languages: Vec<String>,

    pub updated_at: time::OffsetDateTime,
}

impl RepoDetails {
    pub fn last_activity(&self) -> Option<&OffsetDateTime> {
        self.last_pushed_at
            .as_ref()
            .or(self.last_pullrequest_merged_at.as_ref())
    }

    pub fn last_activity_relative_time(&self) -> Option<String> {
        let time = self.last_activity()?;
        let elapsed = OffsetDateTime::now_utc() - *time;

        let days = elapsed.whole_days();

        let v = if days < 1 {
            "today".to_string()
        } else if days < 2 {
            "yesterday".to_string()
        } else if days < 7 {
            format!("{} days", days)
        } else if days < 14 {
            "1 week".to_string()
        } else if days < 30 {
            format!("{} weeks", days / 7)
        } else if days < 60 {
            "1 month".to_string()
        } else if days < 365 {
            format!("{} months", days / 30)
        } else if days < 365 * 2 {
            "1 year".to_string()
        } else {
            format!("{} years", days / 365)
        };

        Some(v)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum RepoDetailsItem {
    Found(RepoDetails),
    NotFound {
        ident: RepoIdent,
        updated_at: time::OffsetDateTime,
    },
}

impl RepoDetailsItem {
    pub fn ident(&self) -> &RepoIdent {
        match self {
            RepoDetailsItem::Found(details) => &details.ident,
            RepoDetailsItem::NotFound { ident, .. } => ident,
        }
    }

    /// Returns `true` if the repo details item is [`NotFound`].
    ///
    /// [`NotFound`]: RepoDetailsItem::NotFound
    #[must_use]
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound { .. })
    }

    /// Returns `true` if the repo details item is [`Found`].
    ///
    /// [`Found`]: RepoDetailsItem::Found
    #[must_use]
    pub fn is_found(&self) -> bool {
        matches!(self, Self::Found(..))
    }

    pub fn updated_at(&self) -> OffsetDateTime {
        match self {
            Self::Found(x) => x.updated_at,
            Self::NotFound { updated_at, .. } => *updated_at,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FullReadmeRepo {
    pub repo: ReadmeRepo,
    pub links: Vec<FullRepoLink>,
    pub not_found: Vec<RepoIdent>,
}

impl FullReadmeRepo {
    pub fn missing_links_count(&self) -> usize {
        self.missing_links().len()
    }

    pub fn has_missing_links(&self) -> bool {
        self.missing_links_count() > 0
    }

    pub fn missing_links(&self) -> Vec<&RepoIdent> {
        let mut links: Vec<_> = self
            .repo
            .repo_links
            .iter()
            .filter(|link| !self.links.iter().any(|l| l.link.ident == link.ident))
            .filter(|link| !self.not_found.contains(&link.ident))
            .map(|link| &link.ident)
            .collect();

        links.sort();
        links.dedup();
        links
    }
}

#[derive(Clone, Debug)]
pub struct FullRepoLink {
    pub link: RepoLink,
    pub details: RepoDetails,
}

#[derive(Clone, Debug)]
pub struct RateLimitError {
    pub message: String,
    #[allow(dead_code)]
    pub reset_at: Option<SystemTime>,
}

impl std::fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Rate limit exceeded: {}", self.message)
    }
}

impl std::error::Error for RateLimitError {}
