pub mod cache;
pub mod config;
pub mod enrichment;
pub mod error;
pub mod ssh_facts;
pub mod types;

pub use config::{CliArgs, FactsConfig};
pub use enrichment::enrich_with_facts;
pub use error::{FactsError, Result};
pub use ssh_facts::{gather_minimal_facts, parse_fact_output};
pub use types::{
    ArchitectureFacts, CachedFact, EnrichedInventory, EnrichedPlaybook, EnrichmentReport,
    FactCache, ParsedInventory, ParsedPlay, ParsedPlaybook, PlaybookMetadata, Task,
};
