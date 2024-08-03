mod routes;
mod ui;

use std::{net::SocketAddr, path::PathBuf, time::Duration};

use anyhow::Context;
use axum::{
    http::StatusCode,
    routing::{get, post},
    Router,
};
use tower_http::trace::TraceLayer;

use crate::{
    loader::Loader,
    source::{github::GithubClient, loader::SourceLoader, RepoIdent},
    storage::{fs::FsStore, Store},
};

pub struct CtxBuilder {
    pub data_dir: PathBuf,
    pub github_token: Option<String>,
}

impl CtxBuilder {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            github_token: None,
        }
    }

    pub fn github_token(mut self, token: Option<String>) -> Self {
        self.github_token = token;
        self
    }

    pub fn build(self) -> Result<Ctx, anyhow::Error> {
        let github = GithubClient::new(self.github_token);
        let sources = SourceLoader::new(github);
        let store = Store::Fs(FsStore::new(self.data_dir)?);

        let loader = Loader::start(store.clone(), sources);

        Ok(Ctx { store, loader })
    }
}

/// Server context.
#[derive(Clone)]
pub struct Ctx {
    #[allow(dead_code)]
    store: Store,
    loader: Loader,
}

impl Ctx {
    pub fn new(store: Store) -> Self {
        let github = GithubClient::from_env();
        let sources = SourceLoader::new(github);
        let loader = Loader::start(store.clone(), sources);

        Ctx { store, loader }
    }

    pub async fn run_server(self, port: u16) -> Result<(), anyhow::Error> {
        let ctx = Ctx::new(self.store);
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        run_server(addr, ctx).await
    }
}

pub const DEFAULT_PORT: u16 = 3333;

fn build_router(ctx: Ctx) -> Router {
    Router::new()
        .route("/", get(routes::homepage::handler_homepage))
        .route(
            routes::search::PATH_SEARCH,
            get(routes::search::handler_search),
        )
        .route(
            routes::repo_list::PATH_README_LIST,
            get(routes::repo_list::handler_readme_list),
        )
        .route(
            "/repo/:source/:owner/:repo",
            get(routes::repo_page::handler_repo),
        )
        // API
        .route(
            routes::api_export::PATH_API_EXPORT,
            get(routes::api_export::handler_api_export),
        )
        .route(
            routes::api_import::PATH_API_IMPORT,
            post(routes::api_import::handler_api_import),
        )
        .with_state(ctx)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(
                    tower_http::trace::DefaultMakeSpan::new().level(tracing::Level::INFO),
                )
                .on_response(
                    tower_http::trace::DefaultOnResponse::new().level(tracing::Level::INFO),
                ),
        )
        .layer(
            // Graceful shutdown will wait for outstanding requests to complete.
            // Add a timeout so requests don't hang forever.
            tower_http::timeout::TimeoutLayer::new(Duration::from_secs(30)),
        )
}

async fn run_server(addr: SocketAddr, ctx: Ctx) -> Result<(), anyhow::Error> {
    tracing::info!("starting server: {}", addr);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("could not bind port")?;

    let app = build_router(ctx);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server failed")
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("received shutdown signal");
}

struct ApiError {
    message: String,
    status: StatusCode,
    source: Option<anyhow::Error>,
}

impl ApiError {
    pub fn msg(message: impl Into<String>, status: StatusCode) -> Self {
        Self {
            message: message.into(),
            status,
            source: None,
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(source: anyhow::Error) -> Self {
        Self {
            message: source.to_string(),
            status: StatusCode::INTERNAL_SERVER_ERROR,
            source: Some(source),
        }
    }
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response<axum::body::Body> {
        let data = serde_json::json!({
            "error": {
                "message": &self.message,
                "source": self.source.as_ref().map(|x| format!("{:#?}", x))
            }
        });

        let body = serde_json::to_vec(&data).unwrap();

        axum::http::Response::builder()
            .status(self.status)
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body))
            .unwrap()
    }
}

struct HtmlError {
    message: String,
    status: StatusCode,
    source: Option<anyhow::Error>,
}

impl HtmlError {
    pub fn msg(message: impl Into<String>, status: StatusCode) -> Self {
        Self {
            message: message.into(),
            status,
            source: None,
        }
    }
}

impl From<anyhow::Error> for HtmlError {
    fn from(source: anyhow::Error) -> Self {
        Self {
            message: source.to_string(),
            status: StatusCode::INTERNAL_SERVER_ERROR,
            source: Some(source),
        }
    }
}

impl axum::response::IntoResponse for HtmlError {
    fn into_response(self) -> axum::response::Response<axum::body::Body> {
        let body = crate::server::ui::render_html_error_standalone(&self);

        axum::http::Response::builder()
            .status(self.status)
            .header("content-type", "text/html")
            .body(axum::body::Body::from(body))
            .unwrap()
    }
}

struct HtmlErrorPage(HtmlError);

impl From<anyhow::Error> for HtmlErrorPage {
    fn from(source: anyhow::Error) -> Self {
        Self(HtmlError::from(source))
    }
}

impl axum::response::IntoResponse for HtmlErrorPage {
    fn into_response(self) -> axum::response::Response<axum::body::Body> {
        let body = crate::server::ui::render_html_error_page(&self.0);

        axum::http::Response::builder()
            .status(self.0.status)
            .header("content-type", "text/html")
            .body(axum::body::Body::from(body))
            .unwrap()
    }
}

fn repo_page_uri(ident: &RepoIdent) -> String {
    format!("/repo/{}/{}/{}", ident.source, ident.owner, ident.repo)
}

#[cfg(test)]
async fn test_client_with_store(store: Store) -> axum_test_helper::TestClient {
    let ctx = Ctx::new(store);
    let app = build_router(ctx);
    axum_test_helper::TestClient::new(app).await
}

#[cfg(test)]
async fn test_client() -> (axum_test_helper::TestClient, tempfile::TempDir) {
    let dir = tempfile::TempDir::new().expect("could not create tmp dir for storage");
    let fs =
        crate::storage::fs::FsStore::new(dir.path().to_owned()).expect("could not create FsStore");
    let store = Store::Fs(fs);

    let client = test_client_with_store(store).await;
    (client, dir)
}
