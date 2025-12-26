use clap::Parser;
use ethrex_storage::{EngineType, Store};
use eyre::Result;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use tracing::{Level, info, warn};
use tracing_subscriber::FmtSubscriber;

mod exporter;

use exporter::{STATE_ENTRY_SIZE_PLAIN, STATE_HEADER_SIZE, STATE_MAGIC, STATE_VERSION};

#[derive(Parser, Debug)]
#[command(name = "ethrex-pir-export")]
#[command(about = "Export UBT state snapshots for PIR database generation")]
#[command(version)]
struct Args {
    /// Path to ethrex data directory
    #[arg(long)]
    datadir: PathBuf,

    /// Block number to export state from (only used for hashed mode)
    /// For plain mode, this is ignored as PLAIN_STORAGE always contains current state
    #[arg(long)]
    block: Option<u64>,

    /// Output file path for the state export
    #[arg(long, short)]
    output: PathBuf,

    /// Export using hashed keys (fallback mode when preimages unavailable)
    /// Note: Hashed mode uses legacy format without PIR2 header
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

    let chain_config = store.get_chain_config();
    let chain_id = chain_config.chain_id;

    let output_file = std::fs::File::create(&args.output)?;
    let mut writer = BufWriter::with_capacity(64 * 1024 * 1024, output_file);

    if args.hashed {
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

        info!("Using hashed keys mode (96-byte records, legacy format without header)");
        let count = exporter::export_hashed(&store, state_root, &mut writer)?;
        info!("Exported {} storage entries", count);
    } else {
        if args.block.is_some() {
            warn!(
                "--block option is ignored in plain mode (PLAIN_STORAGE always contains current state)"
            );
        }

        let latest_block_number = store.get_latest_block_number().await?;

        let latest_header = store
            .get_block_header(latest_block_number)?
            .ok_or_else(|| eyre::eyre!("Latest block {} not found", latest_block_number))?;

        let block_hash = latest_header.hash();

        info!(
            "Exporting current state (as of block {})",
            latest_block_number
        );
        info!("Using plain keys mode (PIR2 format, 84-byte records)");

        let count = exporter::export_plain(
            &store,
            latest_block_number,
            chain_id,
            block_hash,
            &mut writer,
        )?;

        info!("--- Export Summary ---");
        info!("Format:       PIR2 v{}", STATE_VERSION);
        info!(
            "Magic:        {:?}",
            std::str::from_utf8(&STATE_MAGIC).unwrap_or("????")
        );
        info!("Header size:  {} bytes", STATE_HEADER_SIZE);
        info!("Entry size:   {} bytes", STATE_ENTRY_SIZE_PLAIN);
        info!("Entry count:  {}", count);
        info!("Block number: {}", latest_block_number);
        info!("Chain ID:     {}", chain_id);
        info!("Block hash:   {:#x}", block_hash);
        info!(
            "Total size:   {} bytes ({:.2} MB)",
            STATE_HEADER_SIZE + (count as usize * STATE_ENTRY_SIZE_PLAIN),
            (STATE_HEADER_SIZE + (count as usize * STATE_ENTRY_SIZE_PLAIN)) as f64 / 1_048_576.0
        );
    }

    writer.flush()?;
    info!("Export complete: {:?}", args.output);

    Ok(())
}
