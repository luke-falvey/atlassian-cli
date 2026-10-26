use clap::Parser;

#[tokio::main]
async fn main() {
    let cli = atlassian_cli::Cli::parse();
    if let Err(err) = atlassian_cli::run(cli).await {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
