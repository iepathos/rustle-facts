use clap::Parser;
use rustle_facts::{enrich_with_facts, CliArgs, EnrichmentReport, FactsConfig};
use std::io::{self, IsTerminal};
use std::process;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();

    init_logging(args.debug);

    if io::stdin().is_terminal() {
        error!("No input provided. This tool expects parsed JSON from stdin.");
        eprintln!("\nUsage: rustle-facts < parsed.json > enriched.json");
        eprintln!("\nExample pipeline:");
        eprintln!("  rustle-parse playbook.yml inventory.yml | rustle-facts | rustle-plan");
        process::exit(1);
    }

    let config: FactsConfig = args.into();
    let config = config.merge_with_env();

    match run_enrichment(config).await {
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

async fn run_enrichment(config: FactsConfig) -> Result<EnrichmentReport, rustle_facts::FactsError> {
    let stdin = io::stdin();
    let stdout = io::stdout();

    enrich_with_facts(stdin.lock(), stdout.lock(), &config).await
}

fn init_logging(debug: bool) {
    let filter = if debug {
        EnvFilter::new("debug")
    } else {
        EnvFilter::from_default_env()
            .add_directive("rustle_facts=info".parse().unwrap())
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(io::stderr)
        .init();
}