use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
#[command(
    name = "rustle-facts",
    about = "Architecture detection for Rustle binary compilation",
    version,
    author
)]
pub struct CliArgs {
    #[arg(long, value_name = "PATH", help = "Path to cache file")]
    pub cache_file: Option<PathBuf>,

    #[arg(
        long,
        value_name = "SECONDS",
        default_value = "86400",
        help = "Cache TTL in seconds"
    )]
    pub cache_ttl: u64,

    #[arg(
        long,
        value_name = "COUNT",
        default_value = "20",
        help = "Max parallel SSH connections"
    )]
    pub parallel: usize,

    #[arg(
        long,
        value_name = "SECONDS",
        default_value = "10",
        help = "SSH timeout per host"
    )]
    pub timeout: u64,

    #[arg(long, help = "Disable caching")]
    pub no_cache: bool,

    #[arg(long, help = "Force refresh all facts regardless of cache")]
    pub force_refresh: bool,

    #[arg(long, value_name = "PATH", help = "Path to SSH config file")]
    pub ssh_config: Option<PathBuf>,

    #[arg(long, help = "Enable debug logging")]
    pub debug: bool,

    #[arg(value_name = "FILE", help = "Input JSON file (use stdin if not provided)")]
    pub input: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactsConfig {
    pub cache_file: PathBuf,
    pub cache_ttl: u64,
    pub parallel_connections: usize,
    pub timeout: u64,
    pub no_cache: bool,
    pub force_refresh: bool,
    pub ssh_config: Option<PathBuf>,
    pub debug: bool,
}

impl Default for FactsConfig {
    fn default() -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rustle");

        Self {
            cache_file: cache_dir.join("arch-facts.json"),
            cache_ttl: 86400,
            parallel_connections: 20,
            timeout: 10,
            no_cache: false,
            force_refresh: false,
            ssh_config: None,
            debug: false,
        }
    }
}

impl From<CliArgs> for FactsConfig {
    fn from(args: CliArgs) -> Self {
        let mut config = FactsConfig::default();

        if let Some(cache_file) = args.cache_file {
            config.cache_file = cache_file;
        }

        config.cache_ttl = args.cache_ttl;
        config.parallel_connections = args.parallel;
        config.timeout = args.timeout;
        config.no_cache = args.no_cache;
        config.force_refresh = args.force_refresh;
        config.ssh_config = args.ssh_config;
        config.debug = args.debug;

        config
    }
}

impl FactsConfig {
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(cache_dir) = std::env::var("RUSTLE_FACTS_CACHE_DIR") {
            config.cache_file = PathBuf::from(cache_dir).join("arch-facts.json");
        }

        if let Ok(ttl) = std::env::var("RUSTLE_FACTS_CACHE_TTL") {
            if let Ok(ttl_secs) = ttl.parse() {
                config.cache_ttl = ttl_secs;
            }
        }

        if let Ok(parallel) = std::env::var("RUSTLE_FACTS_PARALLEL") {
            if let Ok(parallel_count) = parallel.parse() {
                config.parallel_connections = parallel_count;
            }
        }

        if let Ok(timeout) = std::env::var("RUSTLE_FACTS_SSH_TIMEOUT") {
            if let Ok(timeout_secs) = timeout.parse() {
                config.timeout = timeout_secs;
            }
        }

        config
    }

    pub fn merge_with_env(mut self) -> Self {
        let env_config = Self::from_env();

        if std::env::var("RUSTLE_FACTS_CACHE_DIR").is_ok() {
            self.cache_file = env_config.cache_file;
        }

        if std::env::var("RUSTLE_FACTS_CACHE_TTL").is_ok() {
            self.cache_ttl = env_config.cache_ttl;
        }

        if std::env::var("RUSTLE_FACTS_PARALLEL").is_ok() {
            self.parallel_connections = env_config.parallel_connections;
        }

        if std::env::var("RUSTLE_FACTS_SSH_TIMEOUT").is_ok() {
            self.timeout = env_config.timeout;
        }

        self
    }
}
