use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, SystemTime},
};

use time::OffsetDateTime;
use tokio::sync::RwLock;

use crate::{
    source::{
        loader::SourceLoader, FullReadmeRepo, RateLimitError, ReadmeRepo, RepoDetailsItem,
        RepoIdent,
    },
    storage::{Storage, Store},
};

#[derive(PartialEq, Eq, Clone, Debug)]
enum Task {
    LoadRepoDetails(RepoIdent),
    LoadReadmeRepo(RepoIdent),
}

#[derive(Clone, Debug)]
struct TaskQueue {
    tasks: Arc<tokio::sync::Mutex<VecDeque<Task>>>,
}

impl TaskQueue {
    fn new() -> Self {
        Self {
            tasks: Arc::new(tokio::sync::Mutex::new(VecDeque::new())),
        }
    }

    async fn push(&self, task: Task) {
        let mut lock = self.tasks.lock().await;
        if !lock.contains(&task) {
            lock.push_back(task);
        }
    }

    async fn push_many(&self, tasks: Vec<Task>) {
        let mut lock = self.tasks.lock().await;
        for task in tasks {
            if !lock.contains(&task) {
                lock.push_back(task);
            }
        }
    }

    async fn pop(&self) -> Option<Task> {
        let mut lock = self.tasks.lock().await;
        lock.pop_front()
    }

    async fn run_task_loop(queue: Self, loader: Loader) -> Result<(), anyhow::Error> {
        loop {
            let Some(task) = queue.pop().await else {
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            };

            match Self::run_task(task.clone(), loader.clone()).await {
                Ok(_) => {
                    tracing::trace!(?task, "task completed");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                Err(e) => {
                    tracing::warn!(?task, "task failed: {}", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    #[tracing::instrument(skip(loader), ret)]
    async fn run_task(task: Task, loader: Loader) -> Result<(), anyhow::Error> {
        tracing::trace!("starting task");

        match task {
            Task::LoadRepoDetails(ident) => loader.load_repo_details(&ident).await.map(|_| ()),
            Task::LoadReadmeRepo(repo) => loader.source_load_readme_repo(&repo).await.map(|_| ()),
        }
    }
}

/// Serves as a bridge between the storage and the sources, and also caches
/// data in memory.
#[derive(Clone)]
pub struct Loader {
    store: Store,
    source: SourceLoader,
    cache: Cache,

    tasks: TaskQueue,

    memory_update_time: Duration,
    readme_storage_refresh_time: Duration,
}

impl Loader {
    pub fn new(store: Store, source: SourceLoader) -> Self {
        Self {
            store,
            source,
            cache: Cache::new(),
            tasks: TaskQueue::new(),
            memory_update_time: Duration::from_secs(60),
            // FIXME: appropriate time
            readme_storage_refresh_time: Duration::from_secs(60 * 30),
        }
    }

    pub fn start(store: Store, source: SourceLoader) -> Loader {
        let s = Self::new(store, source);
        tokio::spawn({
            let s = s.clone();
            async move {
                match TaskQueue::run_task_loop(s.tasks.clone(), s.clone()).await {
                    Ok(_) => {
                        tracing::debug!("task loop finished gracefully");
                    }
                    Err(e) => {
                        tracing::error!("task loop failed: {}", e);
                    }
                }
            }
        });
        s
    }

    async fn source_load_repo_details(
        &self,
        ident: &RepoIdent,
    ) -> Result<RepoDetailsItem, anyhow::Error> {
        let details = self.source.load_repo_details(ident).await?;
        self.store.repo_details_upsert(details.clone()).await?;
        Ok(details)
    }

    async fn load_repo_details(&self, ident: &RepoIdent) -> Result<RepoDetailsItem, anyhow::Error> {
        if let Some(d) = self.store.repo_details(ident.clone()).await? {
            Ok(d)
        } else {
            self.source_load_repo_details(ident).await
        }
    }

    async fn source_load_readme_repo(
        &self,
        ident: &RepoIdent,
    ) -> Result<ReadmeRepo, anyhow::Error> {
        let repo = self.source.load_readme_repo(ident).await?;
        self.store.readme_repo_upsert(repo.clone()).await?;
        Ok(repo)
    }

    async fn load_readme_repo(&self, ident: &RepoIdent) -> Result<ReadmeRepo, anyhow::Error> {
        if let Some(r) = self.store.readme_repo(ident.clone()).await? {
            Ok(r)
        } else {
            self.source_load_readme_repo(ident).await
        }
    }

    pub async fn load_full_readme_repo(
        &self,
        ident: RepoIdent,
        allow_source_refresh: bool,
    ) -> Result<Arc<FullReadmeRepo>, anyhow::Error> {
        tracing::trace!("loading full readme repo for {}", ident);
        let repo_opt = self
            .cache
            .readme_repo(&ident)
            .await
            // Refresh if expired.
            .filter(|x| x.inserted_at.elapsed().unwrap_or_default() > self.memory_update_time);

        if repo_opt.is_none() {
            let repo = self.load_readme_repo(&ident).await?;
            let mut not_found_repos = Vec::new();

            let mut links = Vec::new();
            for link in &repo.repo_links {
                // Ignore links to the same repo.
                if link.ident == ident {
                    continue;
                }

                let details = if allow_source_refresh {
                    self.load_repo_details(&link.ident).await
                } else if let Some(x) = self.store.repo_details(ident.clone()).await? {
                    Ok(x)
                } else {
                    continue;
                };

                match details {
                    Ok(d) => {
                        match d {
                            RepoDetailsItem::Found(details) => {
                                links.push(crate::source::FullRepoLink {
                                    link: link.clone(),
                                    details,
                                });
                            }
                            RepoDetailsItem::NotFound { .. } => {
                                // TODO: queue refresh?
                                not_found_repos.push(link.ident.clone());
                            }
                        }
                    }
                    Err(e) if e.is::<RateLimitError>() => {
                        tracing::warn!("rate limit exceeded: {}", e);
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("failed to load repo details: {}", e);
                    }
                };
            }

            let full_repo = FullReadmeRepo {
                repo,
                links,
                not_found: not_found_repos,
            };
            self.cache
                .readme_repo_insert(ident.clone(), full_repo.clone())
                .await;
        }

        let repo = self.cache.readme_repo(&ident).await.unwrap();

        // Queue tasks for missing repos.
        {
            let missing_links = repo.data.missing_links();

            tracing::trace!(?missing_links, "scheduling tasks for missing repos");

            let tasks: Vec<_> = missing_links
                .iter()
                .map(|ident| Task::LoadRepoDetails((*ident).clone()))
                .collect();
            self.tasks.push_many(tasks).await;
        }
        // Queue task for readme refresh.
        if (OffsetDateTime::now_utc() - repo.data.repo.updated_at)
            > self.readme_storage_refresh_time
        {
            self.tasks.push(Task::LoadReadmeRepo(ident.clone())).await;
        }

        Ok(repo.data)
    }

    #[tracing::instrument(skip_all)]
    pub async fn popular_repos(
        &self,
        count: usize,
    ) -> Result<Vec<Arc<FullReadmeRepo>>, anyhow::Error> {
        tracing::trace!("loading populer repos");
        // FIXME: add caching!

        let mut repos = self.store.readme_repo_list().await?;
        repos.sort_by_key(|r| r.details.stargazer_count);
        repos.truncate(count);

        let mut full_repos = Vec::new();
        for repo in repos {
            let full_repo = self
                .load_full_readme_repo(repo.details.ident.clone(), false)
                .await?;
            full_repos.push(full_repo);
        }

        full_repos.sort_by(|a, b| {
            b.repo
                .details
                .stargazer_count
                .cmp(&a.repo.details.stargazer_count)
        });

        tracing::trace!("popular repos loaded ({})", full_repos.len());

        Ok(full_repos)
    }
}

#[derive(Clone)]
struct CacheEntry<T> {
    data: T,
    inserted_at: SystemTime,
}

#[derive(Clone)]
struct Cache {
    readme_repos: Arc<RwLock<HashMap<RepoIdent, CacheEntry<Arc<FullReadmeRepo>>>>>,
}

impl Cache {
    fn new() -> Self {
        Self {
            readme_repos: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn readme_repo(&self, ident: &RepoIdent) -> Option<CacheEntry<Arc<FullReadmeRepo>>> {
        self.readme_repos.read().await.get(ident).cloned()
    }

    async fn readme_repo_insert(&self, ident: RepoIdent, data: FullReadmeRepo) {
        self.readme_repos.write().await.insert(
            ident,
            CacheEntry {
                data: Arc::new(data),
                inserted_at: SystemTime::now(),
            },
        );
    }
}
