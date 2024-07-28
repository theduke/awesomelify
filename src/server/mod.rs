mod routes;
mod ui;

use std::{net::SocketAddr, time::Duration};

use anyhow::Context;
use axum::{http::StatusCode, routing::get, Router};
use tower_http::trace::TraceLayer;

use crate::{
    loader::Loader,
    source::{github::GithubClient, loader::SourceLoader, RepoIdent},
    storage::Store,
};

pub struct ServerBuilder {
    pub port: u16,
    pub store: Store,
}

impl ServerBuilder {
    pub fn new(store: Store) -> Self {
        Self {
            port: DEFAULT_PORT,
            store,
        }
    }

    pub async fn run(self) -> Result<(), anyhow::Error> {
        let github = GithubClient::from_env();
        let sources = SourceLoader::new(github);
        let loader = Loader::start(self.store.clone(), sources);

        let ctx = Ctx {
            store: self.store,
            loader,
        };

        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        run_server(addr, ctx).await
    }
}

const DEFAULT_PORT: u16 = 3333;

/// Server context.
#[derive(Clone)]
struct Ctx {
    #[allow(dead_code)]
    store: Store,
    loader: Loader,
}

async fn run_server(addr: SocketAddr, ctx: Ctx) -> Result<(), anyhow::Error> {
    let app = Router::new()
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
        );

    tracing::info!("starting server: {}", addr);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("could not bind port")?;

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
