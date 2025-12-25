use clap::Parser;
use ethrex_storage::{EngineType, Store};
use eyre::Result;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

mod exporter;

#[derive(Parser, Debug)]
#[command(name = "ethrex-pir-export")]
#[command(about = "Export UBT state snapshots for PIR database generation")]
#[command(version)]
struct Args {
    /// Path to ethrex data directory
    #[arg(long)]
    datadir: PathBuf,

    /// Block number to export state from (defaults to latest finalized)
    #[arg(long)]
    block: Option<u64>,

    /// Output file path for the state export
    #[arg(long, short)]
    output: PathBuf,

    /// Export using hashed keys (fallback mode when preimages unavailable)
    #[arg(long, default_value = "false")]
    hashed: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let args = Args::parse();

    info!("Opening store at {:?}", args.datadir);
    let store = Store::new(&args.datadir, EngineType::RocksDB)?;

    let block_number = match args.block {
        Some(n) => n,
        None => {
            let finalized = store
                .get_finalized_block_number()
                .await?
                .ok_or_else(|| eyre::eyre!("No finalized block found"))?;
            info!("Using latest finalized block: {}", finalized);
            finalized
        }
    };

    let header = store
        .get_block_header(block_number)?
        .ok_or_else(|| eyre::eyre!("Block {} not found", block_number))?;

    let state_root = header.state_root;
    info!(
        "Exporting state at block {} with state_root {:?}",
        block_number, state_root
    );

    let output_file = std::fs::File::create(&args.output)?;
    let mut writer = BufWriter::with_capacity(64 * 1024 * 1024, output_file);

    if args.hashed {
        info!("Using hashed keys mode (96-byte records)");
        let count = exporter::export_hashed(&store, state_root, &mut writer)?;
        info!("Exported {} storage entries", count);
    } else {
        info!("Using plain keys mode (84-byte records)");
        info!("Note: Plain mode exports current state, not historical state at block {}", block_number);
        let count = exporter::export_plain(&store, &mut writer)?;
        info!("Exported {} storage entries", count);
    }

    writer.flush()?;
    info!("Export complete: {:?}", args.output);

    Ok(())
}
