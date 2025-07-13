use crate::error::{FactsError, Result};
use crate::ssh_facts::generate_ssh_fingerprint;
use crate::types::{ArchitectureFacts, CachedFact, FactCache};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

impl FactCache {
    pub fn get(&self, host: &str, ttl: u64) -> Option<&ArchitectureFacts> {
        self.facts
            .get(host)
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

    pub fn merge_facts(&mut self, new_facts: &HashMap<String, ArchitectureFacts>) {
        for (host, facts) in new_facts {
            self.update(host.clone(), facts.clone());
        }
    }

    pub fn cleanup_stale(&mut self, ttl: u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.facts.retain(|host, cached| {
            let is_valid = (now - cached.timestamp) < ttl as i64;
            if !is_valid {
                debug!("Removing stale cache entry for host: {}", host);
            }
            is_valid
        });
    }
}

pub fn is_cache_valid(fact: &CachedFact, ttl: u64) -> bool {
    if ttl == 0 {
        return false;
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    (now - fact.timestamp) < ttl as i64
}

pub fn load_cache(path: &Path) -> Result<FactCache> {
    match fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(cache) => {
                info!("Loaded cache from {:?}", path);
                Ok(cache)
            }
            Err(e) => {
                warn!("Cache file corrupted: {}, creating new cache", e);
                Ok(FactCache::new())
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            debug!("Cache file not found, creating new cache");
            Ok(FactCache::new())
        }
        Err(e) => Err(FactsError::CacheError(format!(
            "Failed to read cache file: {}",
            e
        ))),
    }
}

pub fn save_cache(path: &Path, cache: &FactCache) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            FactsError::CacheError(format!("Failed to create cache directory: {}", e))
        })?;
    }

    let json = serde_json::to_string_pretty(cache)?;

    fs::write(path, json).map_err(|e| {
        FactsError::CacheError(format!("Failed to write cache file: {}", e))
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(path)?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(path, permissions)?;
    }

    info!("Saved cache to {:?}", path);
    Ok(())
}

pub fn load_or_create_cache(path: &Path) -> Result<FactCache> {
    load_cache(path)
}

pub fn update_cache(
    cache: &mut FactCache,
    new_facts: &HashMap<String, ArchitectureFacts>,
) -> Result<()> {
    cache.merge_facts(new_facts);
    Ok(())
}

pub fn filter_hosts_needing_facts(
    hosts: &[String],
    cache: &FactCache,
    ttl: u64,
    force_refresh: bool,
) -> Vec<String> {
    if force_refresh {
        return hosts.to_vec();
    }

    hosts
        .iter()
        .filter(|host| cache.get(host, ttl).is_none())
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_cache_validity() {
        let fact = CachedFact {
            facts: ArchitectureFacts::fallback(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            ssh_fingerprint: "test".to_string(),
        };

        assert!(is_cache_valid(&fact, 3600));
        assert!(!is_cache_valid(&fact, 0));

        let old_fact = CachedFact {
            facts: ArchitectureFacts::fallback(),
            timestamp: 1000,
            ssh_fingerprint: "test".to_string(),
        };

        assert!(!is_cache_valid(&old_fact, 3600));
    }

    #[test]
    fn test_cache_operations() {
        let mut cache = FactCache::new();

        let facts = ArchitectureFacts {
            ansible_architecture: "x86_64".to_string(),
            ansible_system: "Linux".to_string(),
            ansible_os_family: "debian".to_string(),
            ansible_distribution: Some("ubuntu".to_string()),
        };

        cache.update("host1".to_string(), facts.clone());

        assert!(cache.get("host1", 3600).is_some());
        assert_eq!(cache.get("host1", 3600).unwrap().ansible_architecture, "x86_64");
        assert!(cache.get("host2", 3600).is_none());
    }

    #[test]
    fn test_cache_persistence() {
        let dir = tempdir().unwrap();
        let cache_path = dir.path().join("test-cache.json");

        let mut cache = FactCache::new();
        cache.update(
            "testhost".to_string(),
            ArchitectureFacts {
                ansible_architecture: "aarch64".to_string(),
                ansible_system: "Linux".to_string(),
                ansible_os_family: "redhat".to_string(),
                ansible_distribution: Some("centos".to_string()),
            },
        );

        save_cache(&cache_path, &cache).unwrap();

        let loaded_cache = load_cache(&cache_path).unwrap();
        assert_eq!(loaded_cache.facts.len(), 1);
        assert!(loaded_cache.get("testhost", 3600).is_some());
    }

    #[test]
    fn test_filter_hosts_needing_facts() {
        let mut cache = FactCache::new();
        cache.update("host1".to_string(), ArchitectureFacts::fallback());

        let hosts = vec![
            "host1".to_string(),
            "host2".to_string(),
            "host3".to_string(),
        ];

        let needed = filter_hosts_needing_facts(&hosts, &cache, 3600, false);
        assert_eq!(needed.len(), 2);
        assert!(needed.contains(&"host2".to_string()));
        assert!(needed.contains(&"host3".to_string()));

        let all_needed = filter_hosts_needing_facts(&hosts, &cache, 3600, true);
        assert_eq!(all_needed.len(), 3);
    }
}