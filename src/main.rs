use clap::Parser;

mod cli;

fn main() {
    let args = cli::Args::parse();
    args.run();
}
