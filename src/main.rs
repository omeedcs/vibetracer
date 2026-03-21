use clap::Parser;

#[derive(Parser)]
#[command(name = "vibetracer", about = "Trace, replay, and rewind AI coding edits")]
struct Cli {
    /// Project directory to watch (defaults to current directory)
    path: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Replay a past session
    Replay { session_id: String },
    /// List past sessions
    Sessions,
    /// Create default config
    Init,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    println!("vibetracer v{}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
