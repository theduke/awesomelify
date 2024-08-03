use std::path::PathBuf;

use tracing_subscriber::EnvFilter;

#[derive(clap::Parser)]
pub struct Args {
    #[clap(subcommand)]
    pub cmd: Cmd,
}

impl Args {
    pub fn run(self) {
        match self.cmd {
            Cmd::Serve(cmd) => cmd.run().unwrap(),
        }
    }
}

#[derive(clap::Subcommand)]
pub enum Cmd {
    Serve(CmdServe),
}

#[derive(clap::Parser)]
pub struct CmdServe {
    #[clap(long, env = "DATA_DIR", default_value = "data")]
    data_dir: PathBuf,

    /// Github token to use for Github API requests.
    #[clap(long, env = "GITHUB_TOKEN")]
    github_token: Option<String>,
}

impl CmdServe {
    #[tokio::main]
    pub async fn run(self) -> Result<(), anyhow::Error> {
        let filter = EnvFilter::try_from_default_env().unwrap_or("info".parse().unwrap());
        tracing_subscriber::fmt().with_env_filter(filter).init();

        awesomelify::server::CtxBuilder::new(self.data_dir)
            .github_token(self.github_token)
            .build()?
            .run_server(awesomelify::server::DEFAULT_PORT)
            .await?;

        Ok(())
    }
}
