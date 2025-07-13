# Specification 010: Rustle-Facts Architecture Detection Tool

## Feature Summary

Rustle-facts is a specialized tool that enriches parsed Ansible playbook data with target host architecture and OS information. It performs minimal SSH operations to gather only the essential facts needed for binary compilation targeting, then adds this information to the parsed JSON data flowing through the Rustle pipeline. This tool bridges the gap between parsed playbooks and the planning phase by ensuring all hosts have the necessary architecture facts for cross-compilation decisions.

## Goals & Requirements

### Functional Requirements
- Accept parsed JSON from rustle-parse via stdin
- Detect and gather architecture facts from target hosts via SSH
- Enrich the parsed JSON with architecture/OS facts for each host
- Output enriched JSON to stdout for consumption by rustle-plan
- Support caching of architecture facts to minimize SSH operations
- Handle multiple hosts in parallel for efficient fact gathering
- Provide fallback mechanisms for hosts that are unreachable

### Non-Functional Requirements
- **Performance**: Complete fact gathering for 100 hosts in under 5 seconds
- **Reliability**: Handle SSH failures gracefully with appropriate error reporting
- **Efficiency**: Minimize SSH operations - only gather what's needed for compilation
- **Compatibility**: Maintain full compatibility with existing Ansible inventory formats
- **Caching**: Support persistent caching with TTL and invalidation

### Success Criteria
- Successfully enriches parsed playbook data with architecture facts
- Reduces SSH operations by 90% compared to full Ansible fact gathering
- Provides accurate architecture detection for cross-compilation
- Integrates seamlessly into the Rustle pipeline
- Handles edge cases and failures gracefully

## API/Interface Design

### Command Line Interface
```rust
// Binary invocation
rustle-facts [OPTIONS] < parsed.json > enriched.json

// Options
--cache-file <PATH>         Path to cache file (default: ~/.rustle/arch-facts.json)
--cache-ttl <SECONDS>       Cache TTL in seconds (default: 86400)
--parallel <COUNT>          Max parallel SSH connections (default: 20)
--timeout <SECONDS>         SSH timeout per host (default: 10)
--no-cache                  Disable caching
--force-refresh             Force refresh all facts regardless of cache
--ssh-config <PATH>         Path to SSH config file
--debug                     Enable debug logging
```

### Core Types
```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureFacts {
    pub ansible_architecture: String,  // x86_64, aarch64, arm64, etc.
    pub ansible_system: String,        // Linux, Darwin, Windows
    pub ansible_os_family: String,     // RedHat, Debian, Alpine, etc.
    pub ansible_distribution: Option<String>, // Ubuntu, CentOS, etc.
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ParsedPlaybook {
    pub metadata: PlaybookMetadata,
    pub plays: Vec<ParsedPlay>,
    pub variables: HashMap<String, serde_json::Value>,
    pub facts_required: bool,
    pub vault_ids: Vec<String>,
    pub inventory: ParsedInventory,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnrichedPlaybook {
    #[serde(flatten)]
    pub playbook: ParsedPlaybook,
    pub inventory: EnrichedInventory,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnrichedInventory {
    #[serde(flatten)]
    pub base: ParsedInventory,
    pub host_facts: HashMap<String, ArchitectureFacts>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FactCache {
    pub version: String,
    pub facts: HashMap<String, CachedFact>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedFact {
    pub facts: ArchitectureFacts,
    pub timestamp: i64,
    pub ssh_fingerprint: String,
}
```

### Public Functions
```rust
/// Main entry point for fact enrichment
pub async fn enrich_with_facts<R: Read, W: Write>(
    input: R,
    output: W,
    config: &FactsConfig,
) -> Result<EnrichmentReport, FactsError>;

/// Gather minimal facts from a set of hosts
pub async fn gather_minimal_facts(
    hosts: &[String],
    config: &FactsConfig,
) -> Result<HashMap<String, ArchitectureFacts>, FactsError>;

/// Load facts from cache
pub fn load_cache(path: &Path) -> Result<FactCache, FactsError>;

/// Save facts to cache
pub fn save_cache(path: &Path, cache: &FactCache) -> Result<(), FactsError>;

/// Validate cached facts freshness
pub fn is_cache_valid(fact: &CachedFact, ttl: u64) -> bool;
```

## File and Package Structure

```
rustle-facts/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI entry point
│   ├── lib.rs               # Library exports
│   ├── enrichment.rs        # Core enrichment logic
│   ├── ssh_facts.rs         # SSH fact gathering
│   ├── cache.rs             # Caching implementation
│   ├── types.rs             # Type definitions
│   ├── error.rs             # Error types
│   └── config.rs            # Configuration handling
├── tests/
│   ├── integration_tests.rs # Integration tests
│   └── fixtures/            # Test data
└── benches/
    └── performance.rs       # Performance benchmarks
```

## Implementation Details

### Phase 1: Input Processing and Analysis
```rust
// enrichment.rs
pub async fn enrich_with_facts<R: Read, W: Write>(
    mut input: R,
    mut output: W,
    config: &FactsConfig,
) -> Result<EnrichmentReport, FactsError> {
    // 1. Parse input JSON
    let mut buffer = Vec::new();
    input.read_to_end(&mut buffer)?;
    let parsed: ParsedPlaybook = serde_json::from_slice(&buffer)?;
    
    // 2. Extract unique hosts from inventory
    let hosts = extract_unique_hosts(&parsed.inventory)?;
    
    // 3. Load cache if enabled
    let mut cache = if !config.no_cache {
        load_or_create_cache(&config.cache_file)?
    } else {
        FactCache::new()
    };
    
    // 4. Determine which hosts need fact gathering
    let hosts_needing_facts = filter_hosts_needing_facts(
        &hosts,
        &cache,
        config.cache_ttl,
        config.force_refresh
    );
    
    // 5. Gather facts for hosts
    let new_facts = if !hosts_needing_facts.is_empty() {
        gather_minimal_facts(&hosts_needing_facts, config).await?
    } else {
        HashMap::new()
    };
    
    // 6. Update cache
    update_cache(&mut cache, &new_facts)?;
    if !config.no_cache {
        save_cache(&config.cache_file, &cache)?;
    }
    
    // 7. Build enriched output
    let enriched = build_enriched_playbook(parsed, &cache, &new_facts)?;
    
    // 8. Write output
    serde_json::to_writer_pretty(&mut output, &enriched)?;
    
    Ok(EnrichmentReport {
        total_hosts: hosts.len(),
        facts_gathered: new_facts.len(),
        cache_hits: hosts.len() - new_facts.len(),
        duration: start.elapsed(),
    })
}
```

### Phase 2: SSH Fact Gathering
```rust
// ssh_facts.rs
pub async fn gather_minimal_facts(
    hosts: &[String],
    config: &FactsConfig,
) -> Result<HashMap<String, ArchitectureFacts>, FactsError> {
    use tokio::task::JoinSet;
    
    let semaphore = Arc::new(Semaphore::new(config.parallel_connections));
    let mut tasks = JoinSet::new();
    
    for host in hosts {
        let host = host.clone();
        let config = config.clone();
        let sem = semaphore.clone();
        
        tasks.spawn(async move {
            let _permit = sem.acquire().await?;
            gather_single_host_facts(&host, &config).await
        });
    }
    
    let mut results = HashMap::new();
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok((host, facts))) => {
                results.insert(host, facts);
            }
            Ok(Err(e)) => {
                warn!("Failed to gather facts: {}", e);
            }
            Err(e) => {
                error!("Task panic: {}", e);
            }
        }
    }
    
    Ok(results)
}

async fn gather_single_host_facts(
    host: &str,
    config: &FactsConfig,
) -> Result<(String, ArchitectureFacts), FactsError> {
    // Build SSH command to gather minimal facts
    let command = build_fact_gathering_command();
    
    // Execute via SSH
    let output = execute_ssh_command(host, &command, config).await?;
    
    // Parse output
    let facts = parse_fact_output(&output)?;
    
    Ok((host.to_string(), facts))
}

fn build_fact_gathering_command() -> String {
    // Efficient command that works across different systems
    r#"
    echo "ARCH=$(uname -m)"
    echo "SYSTEM=$(uname -s)"
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        echo "OS_FAMILY=$ID_LIKE"
        echo "DISTRIBUTION=$ID"
    elif [ -f /etc/redhat-release ]; then
        echo "OS_FAMILY=rhel"
        echo "DISTRIBUTION=rhel"
    elif [ "$(uname -s)" = "Darwin" ]; then
        echo "OS_FAMILY=darwin"
        echo "DISTRIBUTION=macos"
    else
        echo "OS_FAMILY=unknown"
        echo "DISTRIBUTION=unknown"
    fi
    "#.to_string()
}
```

### Phase 3: Cache Management
```rust
// cache.rs
impl FactCache {
    pub fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            facts: HashMap::new(),
        }
    }
    
    pub fn get(&self, host: &str, ttl: u64) -> Option<&ArchitectureFacts> {
        self.facts.get(host)
            .filter(|cached| is_cache_valid(cached, ttl))
            .map(|cached| &cached.facts)
    }
    
    pub fn update(&mut self, host: String, facts: ArchitectureFacts) {
        let cached = CachedFact {
            facts,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            ssh_fingerprint: generate_ssh_fingerprint(&host),
        };
        self.facts.insert(host, cached);
    }
}

pub fn is_cache_valid(fact: &CachedFact, ttl: u64) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    (now - fact.timestamp) < ttl as i64
}
```

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_fact_output() {
        let output = r#"
        ARCH=x86_64
        SYSTEM=Linux
        OS_FAMILY=debian
        DISTRIBUTION=ubuntu
        "#;
        
        let facts = parse_fact_output(output).unwrap();
        assert_eq!(facts.ansible_architecture, "x86_64");
        assert_eq!(facts.ansible_system, "Linux");
        assert_eq!(facts.ansible_os_family, "debian");
        assert_eq!(facts.ansible_distribution, Some("ubuntu".to_string()));
    }
    
    #[test]
    fn test_cache_validity() {
        let fact = CachedFact {
            facts: test_facts(),
            timestamp: 1000,
            ssh_fingerprint: "test".to_string(),
        };
        
        assert!(is_cache_valid(&fact, 3600)); // Valid within TTL
        assert!(!is_cache_valid(&fact, 0));   // Invalid with 0 TTL
    }
    
    #[tokio::test]
    async fn test_enrichment_pipeline() {
        let input = include_str!("fixtures/parsed_playbook.json");
        let mut output = Vec::new();
        
        let config = FactsConfig::default();
        let report = enrich_with_facts(
            input.as_bytes(),
            &mut output,
            &config
        ).await.unwrap();
        
        assert!(report.total_hosts > 0);
        
        let enriched: EnrichedPlaybook = serde_json::from_slice(&output).unwrap();
        assert!(!enriched.inventory.host_facts.is_empty());
    }
}
```

### Integration Tests
```rust
// tests/integration_tests.rs
#[tokio::test]
async fn test_real_ssh_gathering() {
    let hosts = vec!["localhost".to_string()];
    let config = FactsConfig::default();
    
    let facts = gather_minimal_facts(&hosts, &config).await.unwrap();
    
    assert_eq!(facts.len(), 1);
    assert!(facts.contains_key("localhost"));
    
    let localhost_facts = &facts["localhost"];
    assert!(!localhost_facts.ansible_architecture.is_empty());
    assert!(!localhost_facts.ansible_system.is_empty());
}

#[tokio::test]
async fn test_pipeline_integration() {
    // Test full pipeline from parsed to enriched
    let parsed = read_test_fixture("parsed_playbook.json");
    let enriched = run_facts_enrichment(parsed).await;
    
    verify_enrichment(&enriched);
}
```

## Edge Cases & Error Handling

### SSH Connection Failures
```rust
match execute_ssh_command(host, &command, config).await {
    Ok(output) => parse_fact_output(&output),
    Err(SshError::ConnectionRefused) => {
        warn!("Host {} unreachable, using fallback facts", host);
        Ok(ArchitectureFacts::fallback())
    }
    Err(SshError::AuthenticationFailed) => {
        error!("Authentication failed for {}", host);
        Err(FactsError::AuthenticationFailed(host))
    }
    Err(e) => Err(e.into()),
}
```

### Architecture Detection Fallbacks
```rust
impl ArchitectureFacts {
    pub fn fallback() -> Self {
        Self {
            ansible_architecture: "x86_64".to_string(),
            ansible_system: "Linux".to_string(),
            ansible_os_family: "debian".to_string(),
            ansible_distribution: None,
        }
    }
    
    pub fn normalize_architecture(arch: &str) -> String {
        match arch {
            "x86_64" | "amd64" => "x86_64",
            "aarch64" | "arm64" => "aarch64",
            "armv7l" | "armhf" => "armv7",
            _ => arch.to_string(),
        }
    }
}
```

### Cache Corruption Handling
```rust
pub fn load_cache(path: &Path) -> Result<FactCache, FactsError> {
    match fs::read_to_string(path) {
        Ok(content) => {
            serde_json::from_str(&content)
                .or_else(|_| {
                    warn!("Cache corrupted, creating new");
                    Ok(FactCache::new())
                })
        }
        Err(_) => Ok(FactCache::new()),
    }
}
```

## Dependencies

### External Crates
```toml
[dependencies]
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
ssh2 = "0.9"
async-ssh2-tokio = "0.8"
tracing = "0.1"
tracing-subscriber = "0.3"
anyhow = "1.0"
thiserror = "1.0"
clap = { version = "4.4", features = ["derive"] }
chrono = "0.4"
```

### Internal Dependencies
- Shared types from the Rustle ecosystem
- Common SSH utilities (if available)
- Logging infrastructure

## Configuration

### Environment Variables
```bash
RUSTLE_FACTS_CACHE_DIR       # Cache directory override
RUSTLE_FACTS_CACHE_TTL       # Default cache TTL in seconds
RUSTLE_FACTS_PARALLEL        # Default parallelism
RUSTLE_FACTS_SSH_TIMEOUT     # SSH timeout override
```

### Configuration File
```toml
# ~/.rustle/facts.toml
[cache]
enabled = true
ttl = 86400
directory = "~/.rustle/cache"

[ssh]
parallel_connections = 20
timeout = 10
retry_count = 3
retry_delay = 1

[logging]
level = "info"
format = "json"
```

## Documentation

### GoDoc Comments
```rust
/// Enriches parsed Ansible playbook data with architecture facts from target hosts.
///
/// This function reads parsed JSON from the input, gathers minimal architecture
/// facts via SSH from hosts that need them, and writes enriched JSON to the output.
///
/// # Arguments
///
/// * `input` - Reader containing parsed playbook JSON
/// * `output` - Writer for enriched playbook JSON
/// * `config` - Configuration for fact gathering behavior
///
/// # Returns
///
/// Returns an EnrichmentReport with statistics about the operation
///
/// # Examples
///
/// ```rust
/// use rustle_facts::{enrich_with_facts, FactsConfig};
/// use std::io::Cursor;
///
/// let input = r#"{"inventory": {"hosts": ["web1", "web2"]}}"#;
/// let mut output = Vec::new();
/// let config = FactsConfig::default();
///
/// let report = enrich_with_facts(
///     Cursor::new(input),
///     &mut output,
///     &config
/// ).await?;
///
/// println!("Gathered facts for {} hosts", report.facts_gathered);
/// ```
pub async fn enrich_with_facts<R: Read, W: Write>(
    input: R,
    output: W,
    config: &FactsConfig,
) -> Result<EnrichmentReport, FactsError>;
```

### CLI Help Text
```
rustle-facts - Architecture detection for Rustle binary compilation

USAGE:
    rustle-facts [OPTIONS] < parsed.json > enriched.json

OPTIONS:
    --cache-file <PATH>      Path to cache file [default: ~/.rustle/arch-facts.json]
    --cache-ttl <SECONDS>    Cache TTL in seconds [default: 86400]
    --parallel <COUNT>       Max parallel SSH connections [default: 20]
    --timeout <SECONDS>      SSH timeout per host [default: 10]
    --no-cache              Disable caching
    --force-refresh         Force refresh all facts regardless of cache
    --ssh-config <PATH>     Path to SSH config file
    --debug                 Enable debug logging
    -h, --help              Print help
    -V, --version           Print version

EXAMPLES:
    # Basic usage in pipeline
    rustle-parse playbook.yml inventory.yml | rustle-facts | rustle-plan

    # With custom cache location
    rustle-facts --cache-file /tmp/facts.json < parsed.json > enriched.json

    # Force refresh all facts
    rustle-facts --force-refresh < parsed.json > enriched.json

    # Disable caching for testing
    rustle-facts --no-cache < parsed.json > enriched.json
```

## Performance Considerations

### Parallel SSH Optimization
- Use connection pooling for multiple commands to same host
- Implement exponential backoff for failed connections
- Batch fact gathering commands to minimize round trips

### Caching Strategy
- Use filesystem-based cache for simplicity and reliability
- Implement cache warming for known host groups
- Support cache preloading from CI/CD pipelines

### Memory Efficiency
- Stream JSON processing for large inventories
- Lazy loading of cache entries
- Efficient data structures for fact storage

## Security Considerations

### SSH Security
- Honor SSH config files and known_hosts
- Support SSH key authentication only (no password auth)
- Validate host keys before executing commands
- Sanitize all command outputs

### Cache Security
- Store cache files with appropriate permissions (0600)
- Include SSH fingerprints in cache for validation
- Support cache encryption for sensitive environments

### Command Injection Prevention
- No user input in SSH commands
- Use static command strings only
- Validate all parsed output before processing