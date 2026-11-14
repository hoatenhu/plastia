mod engine;
mod storage;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Parser)]
#[command(
    name = "plastia",
    version,
    about = "Automatic memory engine for AI agents"
)]
struct Cli {
    #[arg(
        long,
        global = true,
        default_value = ".plastia",
        help = "Memory store directory"
    )]
    home: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a local memory store
    Init,
    /// Ingest a JSON conversation/event batch (array, single object, or {events:[...]})
    Ingest { file: PathBuf },
    /// Find active memories and raw events
    Search {
        query: String,
        #[arg(long)]
        scope: Option<String>,
        #[arg(long, default_value_t = 10)]
        limit: u32,
    },
    /// Incrementally extract durable memories and flag conflicts
    Consolidate {
        #[arg(long)]
        dry_run: bool,
    },
    /// Rebuild the memory index from Markdown files
    Reindex,
    /// Show store counts
    Stats,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    if matches!(&cli.command, Command::Init) {
        let conn = storage::open(&cli.home)?;
        drop(conn);
        println!("Initialized {}", cli.home.display());
        return Ok(());
    }
    if !cli.home.join("memory.sqlite3").exists() {
        return Err("store not initialized; run `plastia init`".into());
    }
    let mut conn = storage::open(&cli.home)?;
    match cli.command {
        Command::Init => unreachable!(),
        Command::Ingest { file } => {
            let n = engine::ingest_file(&mut conn, &file)?;
            println!("Ingested {n} new events");
        }
        Command::Search {
            query,
            scope,
            limit,
        } => {
            let results = engine::search(&conn, &query, scope.as_deref(), limit.min(100))?;
            for item in results {
                println!(
                    "[{}] {} ({})\n  {}",
                    item.kind, item.id, item.scope, item.text
                );
            }
        }
        Command::Consolidate { dry_run } => {
            let summary = engine::consolidate(&mut conn, &cli.home, dry_run)?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Command::Reindex => {
            let n = engine::reindex(&mut conn, &cli.home)?;
            println!("Indexed {n} Markdown memories");
        }
        Command::Stats => {
            println!("{}", serde_json::to_string_pretty(&engine::stats(&conn)?)?);
        }
    }
    Ok(())
}
