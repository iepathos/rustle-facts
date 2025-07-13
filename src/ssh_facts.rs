use crate::config::FactsConfig;
use crate::error::{FactsError, Result};
use crate::types::ArchitectureFacts;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

pub async fn gather_minimal_facts(
    hosts: &[String],
    config: &FactsConfig,
) -> Result<HashMap<String, ArchitectureFacts>> {
    let semaphore = Arc::new(Semaphore::new(config.parallel_connections));
    let mut tasks = JoinSet::new();

    for host in hosts {
        let host = host.clone();
        let config = config.clone();
        let sem = semaphore.clone();

        tasks.spawn(async move {
            let _permit = sem.acquire().await.map_err(|e| {
                FactsError::TaskJoin(format!("Failed to acquire semaphore: {}", e))
            })?;

            match timeout(
                Duration::from_secs(config.timeout),
                gather_single_host_facts(&host, &config),
            )
            .await
            {
                Ok(Ok((h, facts))) => Ok((h, facts)),
                Ok(Err(e)) => {
                    warn!("Failed to gather facts from {}: {}", host, e);
                    Err(e)
                }
                Err(_) => {
                    warn!("Timeout gathering facts from {}", host);
                    Err(FactsError::Timeout(host))
                }
            }
        });
    }

    let mut results = HashMap::new();
    let mut failed_hosts = Vec::new();

    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok((host, facts))) => {
                info!("Successfully gathered facts from {}", host);
                results.insert(host, facts);
            }
            Ok(Err(e)) => {
                error!("Error gathering facts: {}", e);
                if let FactsError::ConnectionFailed(host, _) = &e {
                    failed_hosts.push(host.clone());
                }
            }
            Err(e) => {
                error!("Task panic: {}", e);
            }
        }
    }

    if !failed_hosts.is_empty() {
        warn!(
            "Failed to gather facts from {} hosts, using fallback facts",
            failed_hosts.len()
        );
        for host in failed_hosts {
            results.insert(host, ArchitectureFacts::fallback());
        }
    }

    Ok(results)
}

async fn gather_single_host_facts(
    host: &str,
    config: &FactsConfig,
) -> Result<(String, ArchitectureFacts)> {
    debug!("Gathering facts from host: {}", host);

    let command = build_fact_gathering_command();

    let output = execute_ssh_command(host, &command, config).await?;

    let facts = parse_fact_output(&output)
        .map_err(|e| FactsError::ParseError(host.to_string(), e.to_string()))?;

    Ok((host.to_string(), facts))
}

async fn execute_ssh_command(
    host: &str,
    command: &str,
    config: &FactsConfig,
) -> Result<String> {
    let ssh_host = if host.contains('@') {
        host.to_string()
    } else {
        format!("{}@{}", get_ssh_user(host), host)
    };

    let mut ssh_cmd = Command::new("ssh");
    ssh_cmd
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .arg("-o")
        .arg("UserKnownHostsFile=/dev/null")
        .arg("-o")
        .arg(format!("ConnectTimeout={}", config.timeout))
        .arg("-o")
        .arg("BatchMode=yes");

    if let Some(ssh_config_path) = &config.ssh_config {
        if ssh_config_path.exists() {
            debug!("Using SSH config file: {:?}", ssh_config_path);
            ssh_cmd.arg("-F").arg(ssh_config_path);
        }
    }

    ssh_cmd
        .arg(ssh_host.clone())
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = ssh_cmd
        .spawn()
        .map_err(|e| FactsError::ConnectionFailed(host.to_string(), e.to_string()))?;

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    if let Some(mut stdout_handle) = child.stdout.take() {
        stdout_handle.read_to_end(&mut stdout).await?;
    }

    if let Some(mut stderr_handle) = child.stderr.take() {
        stderr_handle.read_to_end(&mut stderr).await?;
    }

    let status = child
        .wait()
        .await
        .map_err(|e| FactsError::ConnectionFailed(host.to_string(), e.to_string()))?;

    if !status.success() {
        let stderr_str = String::from_utf8_lossy(&stderr);
        return Err(FactsError::ConnectionFailed(
            host.to_string(),
            format!("Command failed with exit status: {} - {}", status, stderr_str),
        ));
    }

    Ok(String::from_utf8_lossy(&stdout).to_string())
}

fn get_ssh_user(host: &str) -> String {
    if host.contains('@') {
        host.split('@').next().unwrap_or("root").to_string()
    } else {
        std::env::var("USER").unwrap_or_else(|_| "root".to_string())
    }
}

fn build_fact_gathering_command() -> String {
    r#"
    echo "ARCH=$(uname -m)"
    echo "SYSTEM=$(uname -s)"
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        echo "OS_FAMILY=${ID_LIKE:-$ID}"
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
    "#
    .trim()
    .to_string()
}

pub fn parse_fact_output(output: &str) -> Result<ArchitectureFacts> {
    let mut facts = HashMap::new();

    for line in output.lines() {
        if let Some((key, value)) = line.split_once('=') {
            facts.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    let architecture = facts
        .get("ARCH")
        .ok_or_else(|| FactsError::ParseError("unknown".to_string(), "Missing ARCH".to_string()))?
        .clone();

    let system = facts
        .get("SYSTEM")
        .ok_or_else(|| {
            FactsError::ParseError("unknown".to_string(), "Missing SYSTEM".to_string())
        })?
        .clone();

    let os_family = facts
        .get("OS_FAMILY")
        .unwrap_or(&"unknown".to_string())
        .clone();

    let distribution = facts.get("DISTRIBUTION").cloned();

    Ok(ArchitectureFacts {
        ansible_architecture: ArchitectureFacts::normalize_architecture(&architecture),
        ansible_system: system,
        ansible_os_family: os_family,
        ansible_distribution: distribution,
    })
}

pub fn generate_ssh_fingerprint(host: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    host.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

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
    fn test_parse_fact_output_darwin() {
        let output = r#"
ARCH=arm64
SYSTEM=Darwin
OS_FAMILY=darwin
DISTRIBUTION=macos
"#;

        let facts = parse_fact_output(output).unwrap();
        assert_eq!(facts.ansible_architecture, "aarch64");
        assert_eq!(facts.ansible_system, "Darwin");
        assert_eq!(facts.ansible_os_family, "darwin");
        assert_eq!(facts.ansible_distribution, Some("macos".to_string()));
    }

    #[test]
    fn test_architecture_normalization() {
        assert_eq!(ArchitectureFacts::normalize_architecture("x86_64"), "x86_64");
        assert_eq!(ArchitectureFacts::normalize_architecture("amd64"), "x86_64");
        assert_eq!(
            ArchitectureFacts::normalize_architecture("aarch64"),
            "aarch64"
        );
        assert_eq!(ArchitectureFacts::normalize_architecture("arm64"), "aarch64");
        assert_eq!(ArchitectureFacts::normalize_architecture("armv7l"), "armv7");
        assert_eq!(ArchitectureFacts::normalize_architecture("armhf"), "armv7");
        assert_eq!(
            ArchitectureFacts::normalize_architecture("custom"),
            "custom"
        );
    }
}