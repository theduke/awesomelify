use std::path::PathBuf;

use anyhow::Context;

use crate::source::{ReadmeRepo, RepoDetailsItem, RepoIdent};

#[derive(Clone, Debug)]
pub struct FsStore {
    root: PathBuf,
}

impl FsStore {
    pub fn new(root: PathBuf) -> Result<Self, anyhow::Error> {
        std::fs::create_dir_all(&root)
            .with_context(|| format!("failed to create root directory: '{}'", root.display()))?;

        Ok(Self { root })
    }

    fn repo_details_dir(&self) -> PathBuf {
        self.root.join("repo_details")
    }

    fn ident_to_storage_name(ident: &RepoIdent) -> String {
        format!("{}:{}:{}.json", ident.source, ident.owner, ident.repo)
    }

    fn repo_details_path(&self, ident: &RepoIdent) -> PathBuf {
        self.repo_details_dir()
            .join(Self::ident_to_storage_name(ident))
    }

    fn readme_repo_dir(&self) -> PathBuf {
        self.root.join("readme_repo")
    }

    fn readme_repo_path(&self, ident: &RepoIdent) -> PathBuf {
        self.readme_repo_dir()
            .join(Self::ident_to_storage_name(ident))
    }

    fn repo_details_sync(
        &self,
        ident: &RepoIdent,
    ) -> Result<Option<RepoDetailsItem>, anyhow::Error> {
        let path = self.repo_details_path(&ident);
        match std::fs::read(&path) {
            Ok(data) => {
                let details = serde_json::from_slice(&data)?;
                Ok(Some(details))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).context(format!("failed to read file: '{}'", path.display())),
        }
    }

    fn repo_details_multi_sync(
        &self,
        idents: Vec<RepoIdent>,
    ) -> Result<Vec<RepoDetailsItem>, anyhow::Error> {
        let mut list = Vec::new();

        for ident in idents {
            match self.repo_details_sync(&ident) {
                Ok(Some(details)) => list.push(details),
                Ok(None) => (),
                Err(e) => {
                    tracing::warn!("failed to load repo details: {}", e);
                }
            }
        }

        Ok(list)
    }
}

impl super::Storage for FsStore {
    async fn repo_details(
        &self,
        ident: RepoIdent,
    ) -> Result<Option<RepoDetailsItem>, anyhow::Error> {
        let s = self.clone();
        tokio::task::spawn_blocking(move || s.repo_details_sync(&ident))
            .await
            .context("failed to spawn blocking task")?
    }

    async fn repo_details_multi(
        &self,
        idents: Vec<RepoIdent>,
    ) -> Result<Vec<RepoDetailsItem>, anyhow::Error> {
        let s = self.clone();
        tokio::task::spawn_blocking(move || s.repo_details_multi_sync(idents))
            .await
            .context("failed to spawn blocking task")?
    }

    async fn repo_details_upsert(&self, details: RepoDetailsItem) -> Result<(), anyhow::Error> {
        let path = self.repo_details_path(details.ident());
        let data = serde_json::to_vec(&details)?;

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("failed to create directory: '{}'", parent.display()))?;
        }

        tokio::fs::write(&path, data)
            .await
            .with_context(|| format!("failed to write file: '{}'", path.display()))?;

        Ok(())
    }

    async fn repo_details_list(&self) -> Result<Vec<RepoDetailsItem>, anyhow::Error> {
        let mut list = Vec::new();
        let dir = self.repo_details_dir();

        let mut iter = tokio::fs::read_dir(&dir)
            .await
            .with_context(|| format!("failed to read directory: '{}'", dir.display()))?;

        while let Some(entry) = iter.next_entry().await? {
            let path = entry.path();
            let is_json = path.extension().map_or(false, |ext| ext == "json");
            if !is_json {
                continue;
            }

            let data = tokio::fs::read(&path)
                .await
                .with_context(|| format!("failed to read file: '{}'", path.display()))?;

            match serde_json::from_slice::<RepoDetailsItem>(&data) {
                Ok(readme) => {
                    list.push(readme);
                }
                Err(e) => {
                    tracing::error!(
                        "failed to parse readme repo json file: '{}': {}",
                        path.display(),
                        e
                    );
                }
            }
        }

        Ok(list)
    }

    async fn readme_repo(&self, ident: RepoIdent) -> Result<Option<ReadmeRepo>, anyhow::Error> {
        let path = self.readme_repo_path(&ident);

        match tokio::fs::read(&path).await {
            Ok(data) => {
                let readme = serde_json::from_slice(&data)?;

                Ok(Some(readme))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).context(format!("failed to read file: '{}'", path.display())),
        }
    }

    async fn readme_repo_upsert(&self, readme: ReadmeRepo) -> Result<(), anyhow::Error> {
        let path = self.readme_repo_path(&readme.details.ident);
        let data = serde_json::to_vec(&readme)?;

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("failed to create directory: '{}'", parent.display()))?;
        }

        tokio::fs::write(&path, data)
            .await
            .with_context(|| format!("failed to write file: '{}'", path.display()))?;

        Ok(())
    }

    async fn readme_repo_list(&self) -> Result<Vec<ReadmeRepo>, anyhow::Error> {
        let mut list = Vec::new();
        let dir = self.readme_repo_dir();

        let mut iter = tokio::fs::read_dir(&dir)
            .await
            .with_context(|| format!("failed to read directory: '{}'", dir.display()))?;

        while let Some(entry) = iter.next_entry().await? {
            let path = entry.path();
            let is_json = path.extension().map_or(false, |ext| ext == "json");
            if !is_json {
                continue;
            }

            let data = tokio::fs::read(&path)
                .await
                .with_context(|| format!("failed to read file: '{}'", path.display()))?;

            match serde_json::from_slice::<ReadmeRepo>(&data) {
                Ok(readme) => {
                    list.push(readme);
                }
                Err(e) => {
                    tracing::error!(
                        "failed to parse readme repo json file: '{}': {}",
                        path.display(),
                        e
                    );
                }
            }
        }

        Ok(list)
    }
}
