pub mod fs;

use std::future::Future;

use crate::source::{ReadmeRepo, RepoDetailsItem, RepoIdent};

pub trait Storage {
    fn repo_details(
        &self,
        ident: RepoIdent,
    ) -> impl Future<Output = Result<Option<RepoDetailsItem>, anyhow::Error>> + Send;

    fn repo_details_multi(
        &self,
        idents: Vec<RepoIdent>,
    ) -> impl Future<Output = Result<Vec<RepoDetailsItem>, anyhow::Error>> + Send;

    fn repo_details_upsert(
        &self,
        details: RepoDetailsItem,
    ) -> impl Future<Output = Result<(), anyhow::Error>> + Send;

    fn repo_details_list(
        &self,
    ) -> impl Future<Output = Result<Vec<RepoDetailsItem>, anyhow::Error>> + Send;

    fn readme_repo(
        &self,
        ident: RepoIdent,
    ) -> impl Future<Output = Result<Option<ReadmeRepo>, anyhow::Error>> + Send;

    fn readme_repo_upsert(
        &self,
        readme: ReadmeRepo,
    ) -> impl Future<Output = Result<(), anyhow::Error>> + Send;

    fn readme_repo_list(
        &self,
    ) -> impl Future<Output = Result<Vec<ReadmeRepo>, anyhow::Error>> + Send;

    fn export(&self) -> impl Future<Output = Result<Vec<Item>, anyhow::Error>> + Send;

    fn import(&self, items: Vec<Item>) -> impl Future<Output = Result<(), anyhow::Error>> + Send;
}

/// Represents any kind of item in storage.
/// Used for imports and exports.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Repo(RepoDetailsItem),
    ReadmeRepo(ReadmeRepo),
}

/// Different storage backends.
///
// Used instead of `Box<dyn Storage>` to allow using `async fn` in trait,
// which isn't object safe.
#[derive(Clone, Debug)]
pub enum Store {
    Fs(fs::FsStore),
}

impl From<fs::FsStore> for Store {
    fn from(fs: fs::FsStore) -> Self {
        Store::Fs(fs)
    }
}

impl Storage for Store {
    async fn repo_details(
        &self,
        ident: RepoIdent,
    ) -> Result<Option<RepoDetailsItem>, anyhow::Error> {
        match self {
            Store::Fs(fs) => fs.repo_details(ident).await,
        }
    }

    async fn repo_details_multi(
        &self,
        idents: Vec<RepoIdent>,
    ) -> Result<Vec<RepoDetailsItem>, anyhow::Error> {
        match self {
            Store::Fs(fs) => fs.repo_details_multi(idents).await,
        }
    }

    async fn repo_details_upsert(&self, details: RepoDetailsItem) -> Result<(), anyhow::Error> {
        match self {
            Store::Fs(fs) => fs.repo_details_upsert(details).await,
        }
    }

    async fn repo_details_list(&self) -> Result<Vec<RepoDetailsItem>, anyhow::Error> {
        match self {
            Store::Fs(fs) => fs.repo_details_list().await,
        }
    }

    async fn readme_repo(&self, ident: RepoIdent) -> Result<Option<ReadmeRepo>, anyhow::Error> {
        match self {
            Store::Fs(fs) => fs.readme_repo(ident).await,
        }
    }

    async fn readme_repo_upsert(&self, readme: ReadmeRepo) -> Result<(), anyhow::Error> {
        match self {
            Store::Fs(fs) => fs.readme_repo_upsert(readme).await,
        }
    }

    async fn readme_repo_list(&self) -> Result<Vec<ReadmeRepo>, anyhow::Error> {
        match self {
            Store::Fs(fs) => fs.readme_repo_list().await,
        }
    }

    async fn export(&self) -> Result<Vec<Item>, anyhow::Error> {
        match self {
            Store::Fs(fs) => fs.export().await,
        }
    }

    async fn import(&self, items: Vec<Item>) -> Result<(), anyhow::Error> {
        match self {
            Store::Fs(fs) => fs.import(items).await,
        }
    }
}
