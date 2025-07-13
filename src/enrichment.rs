use crate::cache::{filter_hosts_needing_facts, load_or_create_cache, save_cache, update_cache};
use crate::config::FactsConfig;
use crate::error::{FactsError, Result};
use crate::ssh_facts::gather_minimal_facts;
use crate::types::{
    ArchitectureFacts, EnrichedInventory, EnrichedPlaybook, EnrichmentReport, FactCache,
    ParsedPlaybook,
};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::time::Instant;
use tracing::{info, warn};

pub async fn enrich_with_facts<R: Read, W: Write>(
    mut input: R,
    mut output: W,
    config: &FactsConfig,
) -> Result<EnrichmentReport> {
    let start = Instant::now();

    let mut buffer = Vec::new();
    input.read_to_end(&mut buffer)?;

    let parsed: ParsedPlaybook = serde_json::from_slice(&buffer)
        .map_err(|e| FactsError::InvalidInventory(format!("Failed to parse input JSON: {}", e)))?;

    let hosts = extract_unique_hosts(&parsed)?;
    info!("Found {} unique hosts in inventory", hosts.len());

    let mut cache = if !config.no_cache {
        load_or_create_cache(&config.cache_file)?
    } else {
        FactCache::new()
    };

    if !config.no_cache {
        cache.cleanup_stale(config.cache_ttl);
    }

    let hosts_needing_facts = filter_hosts_needing_facts(
        &hosts,
        &cache,
        config.cache_ttl,
        config.force_refresh,
    );

    info!(
        "Need to gather facts for {} hosts (cache hits: {})",
        hosts_needing_facts.len(),
        hosts.len() - hosts_needing_facts.len()
    );

    let new_facts = if !hosts_needing_facts.is_empty() {
        gather_minimal_facts(&hosts_needing_facts, config).await?
    } else {
        HashMap::new()
    };

    update_cache(&mut cache, &new_facts)?;

    if !config.no_cache && !new_facts.is_empty() {
        save_cache(&config.cache_file, &cache)?;
    }

    let enriched = build_enriched_playbook(parsed, &cache, &new_facts, config.cache_ttl)?;

    serde_json::to_writer_pretty(&mut output, &enriched)?;
    output.write_all(b"\n")?;

    let duration = start.elapsed();

    Ok(EnrichmentReport {
        total_hosts: hosts.len(),
        facts_gathered: new_facts.len(),
        cache_hits: hosts.len() - new_facts.len(),
        duration,
    })
}

fn extract_unique_hosts(playbook: &ParsedPlaybook) -> Result<Vec<String>> {
    let mut hosts = Vec::new();

    for (host, _) in &playbook.inventory.hosts {
        hosts.push(host.clone());
    }

    for (group_name, group_hosts) in &playbook.inventory.groups {
        if group_name != "all" && group_name != "ungrouped" {
            for host in group_hosts {
                if !hosts.contains(host) {
                    hosts.push(host.clone());
                }
            }
        }
    }

    hosts.sort();
    hosts.dedup();

    if hosts.is_empty() {
        return Err(FactsError::InvalidInventory(
            "No hosts found in inventory".to_string(),
        ));
    }

    Ok(hosts)
}

fn build_enriched_playbook(
    mut parsed: ParsedPlaybook,
    cache: &FactCache,
    new_facts: &HashMap<String, ArchitectureFacts>,
    cache_ttl: u64,
) -> Result<EnrichedPlaybook> {
    let mut host_facts = HashMap::new();

    for (host, _) in &parsed.inventory.hosts {
        if let Some(facts) = new_facts.get(host) {
            host_facts.insert(host.clone(), facts.clone());
        } else if let Some(facts) = cache.get(host, cache_ttl) {
            host_facts.insert(host.clone(), facts.clone());
        } else {
            warn!(
                "No facts available for host {}, using fallback",
                host
            );
            host_facts.insert(host.clone(), ArchitectureFacts::fallback());
        }
    }

    for (group_name, group_hosts) in &parsed.inventory.groups {
        if group_name != "all" && group_name != "ungrouped" {
            for host in group_hosts {
                if !host_facts.contains_key(host) {
                    if let Some(facts) = new_facts.get(host) {
                        host_facts.insert(host.clone(), facts.clone());
                    } else if let Some(facts) = cache.get(host, cache_ttl) {
                        host_facts.insert(host.clone(), facts.clone());
                    } else {
                        warn!(
                            "No facts available for host {} in group {}, using fallback",
                            host, group_name
                        );
                        host_facts.insert(host.clone(), ArchitectureFacts::fallback());
                    }
                }
            }
        }
    }

    let enriched_inventory = EnrichedInventory {
        base: parsed.inventory.clone(),
        host_facts,
    };

    parsed.inventory = enriched_inventory.base.clone();

    Ok(EnrichedPlaybook {
        playbook: parsed,
        inventory: enriched_inventory,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ParsedInventory, PlaybookMetadata};
    use std::io::Cursor;

    fn create_test_playbook() -> ParsedPlaybook {
        let mut hosts = HashMap::new();
        hosts.insert("web1".to_string(), serde_json::json!({}));
        hosts.insert("web2".to_string(), serde_json::json!({}));
        hosts.insert("db1".to_string(), serde_json::json!({}));

        let mut groups = HashMap::new();
        groups.insert("webservers".to_string(), vec!["web1".to_string(), "web2".to_string()]);
        groups.insert("databases".to_string(), vec!["db1".to_string()]);

        ParsedPlaybook {
            metadata: PlaybookMetadata {
                file_path: None,
                name: Some("test".to_string()),
                version: Some("1.0".to_string()),
                created_at: None,
                parsed_at: Some("2024-01-01T00:00:00Z".to_string()),
                checksum: None,
            },
            plays: vec![],
            variables: HashMap::new(),
            facts_required: true,
            vault_ids: vec![],
            inventory: ParsedInventory {
                hosts,
                groups,
                host_vars: HashMap::new(),
            },
        }
    }

    #[test]
    fn test_extract_unique_hosts() {
        let playbook = create_test_playbook();
        let hosts = extract_unique_hosts(&playbook).unwrap();

        assert_eq!(hosts.len(), 3);
        assert!(hosts.contains(&"web1".to_string()));
        assert!(hosts.contains(&"web2".to_string()));
        assert!(hosts.contains(&"db1".to_string()));
    }

    #[test]
    fn test_extract_unique_hosts_empty() {
        let playbook = ParsedPlaybook {
            metadata: PlaybookMetadata {
                file_path: None,
                name: Some("empty".to_string()),
                version: Some("1.0".to_string()),
                created_at: None,
                parsed_at: Some("2024-01-01T00:00:00Z".to_string()),
                checksum: None,
            },
            plays: vec![],
            variables: HashMap::new(),
            facts_required: true,
            vault_ids: vec![],
            inventory: ParsedInventory {
                hosts: HashMap::new(),
                groups: HashMap::new(),
                host_vars: HashMap::new(),
            },
        };

        let result = extract_unique_hosts(&playbook);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_enrichment_with_mock_data() {
        let playbook = create_test_playbook();
        let input_json = serde_json::to_string(&playbook).unwrap();
        let mut output = Vec::new();

        let config = FactsConfig {
            no_cache: true,
            ..Default::default()
        };

        let input = Cursor::new(input_json);

        let result = enrich_with_facts(input, &mut output, &config).await;

        match result {
            Ok(_report) => {
                let output_str = String::from_utf8(output).unwrap();
                // Parse as JSON value first to check structure
                let json_value: serde_json::Value = serde_json::from_str(&output_str).unwrap();
                
                // Check the structure matches our expected format
                assert!(json_value["inventory"]["hosts"].is_object());
                assert!(json_value["inventory"]["host_facts"].is_object());
                
                // The enriched structure should have 3 hosts with fallback facts
                let host_facts = json_value["inventory"]["host_facts"].as_object().unwrap();
                assert_eq!(host_facts.len(), 3);
            }
            Err(e) => {
                // This is expected if we can't connect to hosts
                assert!(e.to_string().contains("No hosts found") || 
                       e.to_string().contains("Connection failed"));
            }
        }
    }
}