use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArchitectureFacts {
    pub ansible_architecture: String,
    pub ansible_system: String,
    pub ansible_os_family: String,
    pub ansible_distribution: Option<String>,
}

impl ArchitectureFacts {
    pub fn fallback() -> Self {
        Self {
            ansible_architecture: "x86_64".to_string(),
            ansible_system: "Linux".to_string(),
            ansible_os_family: "debian".to_string(),
            ansible_distribution: None,
        }
    }

    pub fn from_local_system() -> Self {
        let architecture = match std::env::consts::ARCH {
            "x86_64" => "x86_64".to_string(),
            "aarch64" => "aarch64".to_string(),
            "arm" => "armv7".to_string(),
            arch => arch.to_string(),
        };

        let (system, os_family, distribution) = match std::env::consts::OS {
            "macos" => ("Darwin".to_string(), "darwin".to_string(), Some("macOS".to_string())),
            "linux" => ("Linux".to_string(), "debian".to_string(), None), // Default to debian family
            "windows" => ("Windows".to_string(), "windows".to_string(), None),
            os => (os.to_string(), "unknown".to_string(), None),
        };

        Self {
            ansible_architecture: architecture,
            ansible_system: system,
            ansible_os_family: os_family,
            ansible_distribution: distribution,
        }
    }

    pub fn normalize_architecture(arch: &str) -> String {
        match arch.to_lowercase().as_str() {
            "x86_64" | "amd64" => "x86_64".to_string(),
            "aarch64" | "arm64" => "aarch64".to_string(),
            "armv7l" | "armhf" => "armv7".to_string(),
            _ => arch.to_string(),
        }
    }

    pub fn is_localhost(hostname: &str) -> bool {
        matches!(hostname, "localhost" | "127.0.0.1" | "::1")
    }

    pub fn should_use_local_detection(hostname: &str, host_vars: &std::collections::HashMap<String, serde_json::Value>) -> bool {
        // Use local detection if it's localhost or if ansible_connection is local
        Self::is_localhost(hostname) || 
        host_vars.get("ansible_connection")
            .and_then(|v| v.as_str())
            .map(|s| s == "local")
            .unwrap_or(false)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlaybookMetadata {
    pub file_path: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub created_at: Option<String>,
    pub parsed_at: Option<String>,
    pub checksum: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: Option<String>,
    pub module: String,
    pub args: serde_json::Value,
    pub vars: HashMap<String, serde_json::Value>,
    pub when: Option<String>,
    pub loop_items: Option<Vec<serde_json::Value>>,
    pub tags: Vec<String>,
    pub notify: Vec<String>,
    pub changed_when: Option<String>,
    pub failed_when: Option<String>,
    pub ignore_errors: bool,
    pub delegate_to: Option<String>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ParsedPlay {
    pub name: Option<String>,
    pub hosts: String,
    pub vars: Option<HashMap<String, serde_json::Value>>,
    pub tasks: Vec<Task>,
    pub handlers: Vec<serde_json::Value>,
    pub roles: Vec<serde_json::Value>,
    pub strategy: Option<String>,
    pub serial: Option<serde_json::Value>,
    pub max_fail_percentage: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostEntry {
    pub name: String,
    pub address: Option<String>,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub vars: HashMap<String, serde_json::Value>,
    pub groups: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupEntry {
    pub name: String,
    pub hosts: Vec<String>,
    pub children: Vec<String>,
    pub vars: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InventoryHosts {
    Simple(HashMap<String, serde_json::Value>),
    Detailed(HashMap<String, HostEntry>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InventoryGroups {
    Simple(HashMap<String, Vec<String>>),
    Detailed(HashMap<String, GroupEntry>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedInventory {
    pub hosts: InventoryHosts,
    pub groups: InventoryGroups,
    #[serde(default)]
    pub host_vars: HashMap<String, HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<HashMap<String, serde_json::Value>>,
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
pub struct EnrichedInventory {
    #[serde(flatten)]
    pub base: ParsedInventory,
    pub host_facts: HashMap<String, ArchitectureFacts>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnrichedPlaybook {
    #[serde(flatten)]
    pub playbook: ParsedPlaybook,
    pub inventory: EnrichedInventory,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FactCache {
    pub version: String,
    pub facts: HashMap<String, CachedFact>,
}

impl FactCache {
    pub fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            facts: HashMap::new(),
        }
    }
}

impl Default for FactCache {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedFact {
    pub facts: ArchitectureFacts,
    pub timestamp: i64,
    pub ssh_fingerprint: String,
}

#[derive(Debug)]
pub struct EnrichmentReport {
    pub total_hosts: usize,
    pub facts_gathered: usize,
    pub cache_hits: usize,
    pub duration: std::time::Duration,
}
