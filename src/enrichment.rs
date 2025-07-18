use crate::cache::{filter_hosts_needing_facts, load_or_create_cache, save_cache, update_cache};
use crate::config::FactsConfig;
use crate::docker_facts;
use crate::error::{FactsError, Result};
use crate::ssh_facts;
use crate::types::{
    ArchitectureFacts, EnrichedInventory, EnrichedPlaybook, EnrichmentReport, FactCache, HostEntry,
    InventoryGroups, InventoryHosts, ParsedPlaybook,
};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::time::Instant;
use tracing::{debug, info, warn};

pub async fn enrich_with_facts<R: Read, W: Write>(
    mut input: R,
    mut output: W,
    config: &FactsConfig,
) -> Result<EnrichmentReport> {
    let start = Instant::now();

    let mut buffer = Vec::new();
    input.read_to_end(&mut buffer)?;

    let parsed: ParsedPlaybook = serde_json::from_slice(&buffer)
        .map_err(|e| FactsError::InvalidInventory(format!("Failed to parse input JSON: {e}")))?;

    let hosts = extract_unique_hosts(&parsed)?;
    let total_hosts = hosts.len();
    info!("Found {} unique hosts in inventory", total_hosts);

    // Debug inventory format
    match &parsed.inventory.hosts {
        InventoryHosts::Simple(_) => info!("Using Simple inventory format"),
        InventoryHosts::Detailed(_) => info!("Using Detailed inventory format"),
    }

    let mut cache = if !config.no_cache {
        load_or_create_cache(&config.cache_file)?
    } else {
        FactCache::new()
    };

    if !config.no_cache {
        cache.cleanup_stale(config.cache_ttl);
    }

    // Convert host names to HostEntry objects
    let host_entries = hosts
        .into_iter()
        .map(|host| {
            let entry = get_host_entry(&host, &parsed.inventory);
            debug!(
                "Created HostEntry for {}: connection={:?}",
                host, entry.connection
            );
            entry
        })
        .collect::<Vec<_>>();

    // Separate hosts by connection type
    let mut local_hosts = Vec::new();
    let mut ssh_hosts = Vec::new();
    let mut docker_hosts = Vec::new();

    for entry in host_entries {
        let connection_type = get_connection_type(&entry);
        debug!(
            "Host {} has connection type: {}",
            entry.name, connection_type
        );
        match connection_type.as_str() {
            "local" => local_hosts.push(entry),
            "docker" => docker_hosts.push(entry),
            _ => ssh_hosts.push(entry), // Default to SSH
        }
    }

    info!(
        "Found {} local hosts, {} SSH hosts, and {} Docker hosts",
        local_hosts.len(),
        ssh_hosts.len(),
        docker_hosts.len()
    );

    // Handle localhost hosts directly
    let mut new_facts = HashMap::new();
    for host in &local_hosts {
        if config.force_refresh || cache.get(&host.name, config.cache_ttl).is_none() {
            info!("Using direct local detection for host {}", host.name);
            new_facts.insert(host.name.clone(), ArchitectureFacts::from_local_system());
        }
    }

    // Handle SSH hosts
    let ssh_host_names: Vec<String> = ssh_hosts.iter().map(|h| h.name.clone()).collect();
    let ssh_hosts_needing_facts = filter_hosts_needing_facts(
        &ssh_host_names,
        &cache,
        config.cache_ttl,
        config.force_refresh,
    );

    info!(
        "Need to gather facts for {} SSH hosts (cache hits: {})",
        ssh_hosts_needing_facts.len(),
        ssh_hosts.len() - ssh_hosts_needing_facts.len()
    );

    if !ssh_hosts_needing_facts.is_empty() {
        let ssh_facts = ssh_facts::gather_minimal_facts(&ssh_hosts_needing_facts, config).await?;
        new_facts.extend(ssh_facts);
    }

    // Handle Docker hosts
    let docker_host_count = docker_hosts.len();
    let docker_hosts_needing_facts: Vec<HostEntry> = docker_hosts
        .into_iter()
        .filter(|host| config.force_refresh || cache.get(&host.name, config.cache_ttl).is_none())
        .collect();

    info!(
        "Need to gather facts for {} Docker hosts (cache hits: {})",
        docker_hosts_needing_facts.len(),
        docker_host_count - docker_hosts_needing_facts.len()
    );

    if !docker_hosts_needing_facts.is_empty() {
        let docker_facts =
            docker_facts::gather_minimal_facts(docker_hosts_needing_facts, config).await?;
        new_facts.extend(docker_facts);
    }

    update_cache(&mut cache, &new_facts)?;

    if !config.no_cache && !new_facts.is_empty() {
        save_cache(&config.cache_file, &cache)?;
    }

    let enriched = build_enriched_playbook(parsed, &cache, &new_facts, config.cache_ttl)?;

    serde_json::to_writer_pretty(&mut output, &enriched)?;
    output.write_all(b"\n")?;

    let duration = start.elapsed();

    Ok(EnrichmentReport {
        total_hosts,
        facts_gathered: new_facts.len(),
        cache_hits: total_hosts - new_facts.len(),
        duration,
    })
}

fn extract_unique_hosts(playbook: &ParsedPlaybook) -> Result<Vec<String>> {
    let mut hosts = Vec::new();

    // Extract hosts from the hosts section
    match &playbook.inventory.hosts {
        InventoryHosts::Simple(simple_hosts) => {
            for host in simple_hosts.keys() {
                hosts.push(host.clone());
            }
        }
        InventoryHosts::Detailed(detailed_hosts) => {
            for host in detailed_hosts.keys() {
                hosts.push(host.clone());
            }
        }
    }

    // Extract hosts from the groups section
    match &playbook.inventory.groups {
        InventoryGroups::Simple(simple_groups) => {
            for (group_name, group_hosts) in simple_groups {
                if group_name != "all" && group_name != "ungrouped" {
                    for host in group_hosts {
                        if !hosts.contains(host) {
                            hosts.push(host.clone());
                        }
                    }
                }
            }
        }
        InventoryGroups::Detailed(detailed_groups) => {
            for (group_name, group_entry) in detailed_groups {
                if group_name != "all" && group_name != "ungrouped" {
                    for host in &group_entry.hosts {
                        if !hosts.contains(host) {
                            hosts.push(host.clone());
                        }
                    }
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

fn get_host_entry(hostname: &str, inventory: &crate::types::ParsedInventory) -> HostEntry {
    match &inventory.hosts {
        InventoryHosts::Detailed(detailed_hosts) => detailed_hosts
            .get(hostname)
            .cloned()
            .unwrap_or_else(|| HostEntry {
                name: hostname.to_string(),
                address: None,
                port: None,
                user: None,
                vars: get_host_vars(inventory, hostname),
                groups: vec![],
                connection: None,
                ssh_private_key_file: None,
                ssh_common_args: None,
                ssh_extra_args: None,
                ssh_pipelining: None,
                connection_timeout: None,
                ansible_become: None,
                become_method: None,
                become_user: None,
                become_flags: None,
            }),
        InventoryHosts::Simple(_) => HostEntry {
            name: hostname.to_string(),
            address: None,
            port: None,
            user: None,
            vars: get_host_vars(inventory, hostname),
            groups: vec![],
            connection: None,
            ssh_private_key_file: None,
            ssh_common_args: None,
            ssh_extra_args: None,
            ssh_pipelining: None,
            connection_timeout: None,
            ansible_become: None,
            become_method: None,
            become_user: None,
            become_flags: None,
        },
    }
}

fn get_connection_type(host: &HostEntry) -> String {
    debug!(
        "Checking connection type for host {}: connection field = {:?}, vars = {:?}",
        host.name, host.connection, host.vars
    );

    // Check explicit connection field
    if let Some(connection) = &host.connection {
        debug!("Using explicit connection field: {}", connection);
        return connection.clone();
    }

    // Check ansible_connection in vars
    if let Some(ansible_connection) = host.vars.get("ansible_connection") {
        if let Some(conn_str) = ansible_connection.as_str() {
            debug!("Using ansible_connection from vars: {}", conn_str);
            return conn_str.to_string();
        }
    }

    // Check if it should use local detection
    if ArchitectureFacts::should_use_local_detection(&host.name, &host.vars) {
        debug!("Using local detection for host {}", host.name);
        return "local".to_string();
    }

    // Default to SSH
    debug!("Defaulting to SSH for host {}", host.name);
    "ssh".to_string()
}

fn get_host_vars(
    parsed_inventory: &crate::types::ParsedInventory,
    hostname: &str,
) -> HashMap<String, serde_json::Value> {
    match &parsed_inventory.hosts {
        InventoryHosts::Simple(simple_hosts) => simple_hosts
            .get(hostname)
            .and_then(|v| v.as_object())
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default(),
        InventoryHosts::Detailed(detailed_hosts) => detailed_hosts
            .get(hostname)
            .map(|host_entry| host_entry.vars.clone())
            .unwrap_or_default(),
    }
}

fn build_enriched_playbook(
    parsed: ParsedPlaybook,
    cache: &FactCache,
    new_facts: &HashMap<String, ArchitectureFacts>,
    cache_ttl: u64,
) -> Result<EnrichedPlaybook> {
    let mut host_facts = HashMap::new();

    // Get all host names from inventory
    let host_names: Vec<String> = match &parsed.inventory.hosts {
        InventoryHosts::Simple(simple_hosts) => simple_hosts.keys().cloned().collect(),
        InventoryHosts::Detailed(detailed_hosts) => detailed_hosts.keys().cloned().collect(),
    };

    for host in &host_names {
        if let Some(facts) = new_facts.get(host) {
            host_facts.insert(host.clone(), facts.clone());
        } else if let Some(facts) = cache.get(host, cache_ttl) {
            host_facts.insert(host.clone(), facts.clone());
        } else {
            let host_vars = get_host_vars(&parsed.inventory, host);
            if ArchitectureFacts::should_use_local_detection(host, &host_vars) {
                info!("Using local system detection for host {}", host);
                host_facts.insert(host.clone(), ArchitectureFacts::from_local_system());
            } else {
                warn!("No facts available for host {}, using fallback", host);
                host_facts.insert(host.clone(), ArchitectureFacts::fallback());
            }
        }
    }

    // Process groups based on format
    match &parsed.inventory.groups {
        InventoryGroups::Simple(simple_groups) => {
            for (group_name, group_hosts) in simple_groups {
                if group_name != "all" && group_name != "ungrouped" {
                    for host in group_hosts {
                        if !host_facts.contains_key(host) {
                            if let Some(facts) = new_facts.get(host) {
                                host_facts.insert(host.clone(), facts.clone());
                            } else if let Some(facts) = cache.get(host, cache_ttl) {
                                host_facts.insert(host.clone(), facts.clone());
                            } else {
                                let host_vars = get_host_vars(&parsed.inventory, host);
                                if ArchitectureFacts::should_use_local_detection(host, &host_vars) {
                                    info!(
                                        "Using local system detection for host {} in group {}",
                                        host, group_name
                                    );
                                    host_facts.insert(
                                        host.clone(),
                                        ArchitectureFacts::from_local_system(),
                                    );
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
            }
        }
        InventoryGroups::Detailed(detailed_groups) => {
            for (group_name, group_entry) in detailed_groups {
                if group_name != "all" && group_name != "ungrouped" {
                    for host in &group_entry.hosts {
                        if !host_facts.contains_key(host) {
                            if let Some(facts) = new_facts.get(host) {
                                host_facts.insert(host.clone(), facts.clone());
                            } else if let Some(facts) = cache.get(host, cache_ttl) {
                                host_facts.insert(host.clone(), facts.clone());
                            } else {
                                let host_vars = get_host_vars(&parsed.inventory, host);
                                if ArchitectureFacts::should_use_local_detection(host, &host_vars) {
                                    info!(
                                        "Using local system detection for host {} in group {}",
                                        host, group_name
                                    );
                                    host_facts.insert(
                                        host.clone(),
                                        ArchitectureFacts::from_local_system(),
                                    );
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
            }
        }
    }

    let enriched_inventory = EnrichedInventory {
        base: parsed.inventory.clone(),
        host_facts,
    };

    Ok(EnrichedPlaybook {
        metadata: parsed.metadata,
        plays: parsed.plays,
        variables: parsed.variables,
        facts_required: parsed.facts_required,
        vault_ids: parsed.vault_ids,
        inventory: enriched_inventory,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{InventoryGroups, InventoryHosts, ParsedInventory, PlaybookMetadata};
    use std::io::Cursor;

    fn create_test_playbook() -> ParsedPlaybook {
        let mut hosts = HashMap::new();
        hosts.insert("web1".to_string(), serde_json::json!({}));
        hosts.insert("web2".to_string(), serde_json::json!({}));
        hosts.insert("db1".to_string(), serde_json::json!({}));

        let mut groups = HashMap::new();
        groups.insert(
            "webservers".to_string(),
            vec!["web1".to_string(), "web2".to_string()],
        );
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
                hosts: InventoryHosts::Simple(hosts),
                groups: InventoryGroups::Simple(groups),
                variables: HashMap::new(),
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
                hosts: InventoryHosts::Simple(HashMap::new()),
                groups: InventoryGroups::Simple(HashMap::new()),
                variables: HashMap::new(),
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
                assert!(
                    e.to_string().contains("No hosts found")
                        || e.to_string().contains("Connection failed")
                );
            }
        }
    }
}
