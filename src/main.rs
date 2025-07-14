use clap::Parser;
use rustle_facts::{enrich_with_facts, CliArgs, EnrichmentReport, FactsConfig};
use std::fs::File;
use std::io::{self, IsTerminal, BufReader};
use std::process;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();

    init_logging(args.debug);

    if args.input.is_none() && io::stdin().is_terminal() {
        error!("No input provided. This tool expects parsed JSON from stdin or a file.");
        eprintln!("\nUsage: ");
        eprintln!("  rustle-facts < parsed.json > enriched.json");
        eprintln!("  rustle-facts parsed.json > enriched.json");
        eprintln!("\nExample pipeline:");
        eprintln!("  rustle-parse playbook.yml inventory.yml | rustle-facts | rustle-plan");
        process::exit(1);
    }

    let input_file = args.input.clone();
    let config: FactsConfig = args.into();
    let config = config.merge_with_env();

    match run_enrichment(config, input_file).await {
        Ok(report) => {
            info!(
                "Enrichment complete: {} hosts processed, {} facts gathered, {} cache hits in {:?}",
                report.total_hosts, report.facts_gathered, report.cache_hits, report.duration
            );
        }
        Err(e) => {
            error!("Failed to enrich playbook: {}", e);
            process::exit(1);
        }
    }
}

async fn run_enrichment(config: FactsConfig, input_file: Option<std::path::PathBuf>) -> Result<EnrichmentReport, rustle_facts::FactsError> {
    let stdout = io::stdout();

    match input_file {
        Some(file_path) => {
            let file = File::open(&file_path)
                .map_err(|e| rustle_facts::FactsError::Io(e))?;
            let reader = BufReader::new(file);
            enrich_with_facts(reader, stdout.lock(), &config).await
        }
        None => {
            let stdin = io::stdin();
            enrich_with_facts(stdin.lock(), stdout.lock(), &config).await
        }
    }
}

fn init_logging(debug: bool) {
    let filter = if debug {
        EnvFilter::new("debug")
    } else {
        EnvFilter::from_default_env().add_directive("rustle_facts=info".parse().unwrap())
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(io::stderr)
        .init();
}
