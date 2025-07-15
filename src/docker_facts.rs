use crate::types::{ArchitectureFacts, HostEntry};
use crate::config::FactsConfig;
use anyhow::Context;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use tracing::{debug, error, instrument};

/// Gather minimal facts for hosts using Docker connections
#[instrument(skip(hosts, config))]
pub async fn gather_minimal_facts(
    hosts: Vec<HostEntry>,
    config: &FactsConfig,
) -> crate::error::Result<HashMap<String, ArchitectureFacts>> {
    let mut facts = HashMap::new();
    let max_concurrent = config.parallel_connections;
    
    // Process hosts in batches to limit concurrent Docker operations
    for chunk in hosts.chunks(max_concurrent) {
        let mut handles = vec![];
        
        for host in chunk {
            let host_clone = host.clone();
            let timeout_secs = config.timeout;
            
            let handle = tokio::spawn(async move {
                match gather_host_facts(&host_clone, timeout_secs).await {
                    Ok(host_facts) => (host_clone.name.clone(), Ok(host_facts)),
                    Err(e) => (host_clone.name.clone(), Err(crate::error::FactsError::ConnectionFailed(
                        host_clone.name.clone(),
                        e.to_string()
                    ))),
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all tasks in this batch to complete
        for handle in handles {
            match handle.await {
                Ok((hostname, result)) => {
                    match result {
                        Ok(host_facts) => {
                            facts.insert(hostname, host_facts);
                        }
                        Err(e) => {
                            error!("Failed to gather facts for {}: {}", hostname, e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("Task panicked: {}", e);
                }
            }
        }
    }
    
    Ok(facts)
}

/// Gather facts for a single host using Docker
#[instrument(skip(host))]
async fn gather_host_facts(host: &HostEntry, timeout_secs: u64) -> anyhow::Result<ArchitectureFacts> {
    let container_name = host.vars.get("ansible_host")
        .and_then(|v| v.as_str())
        .or(host.address.as_deref())
        .ok_or_else(|| anyhow::anyhow!("No container name found for host {}", host.name))?;
    
    debug!("Gathering facts for Docker container: {}", container_name);
    
    // First check if container is running
    check_container_running(container_name, timeout_secs).await
        .with_context(|| format!("Container {} is not running or accessible", container_name))?;
    
    // Gather facts in parallel
    let (os_type, _hostname, _kernel, _cpu_info) = tokio::try_join!(
        get_os_type(container_name, timeout_secs),
        get_hostname(container_name, timeout_secs),
        get_kernel_info(container_name, timeout_secs),
        get_cpu_info(container_name, timeout_secs)
    )?;
    
    let architecture = get_architecture(container_name, timeout_secs).await?;
    let distribution = get_distribution(container_name, timeout_secs, &os_type).await.ok();
    let os_family = get_os_family(&os_type, &distribution);
    
    Ok(ArchitectureFacts {
        ansible_architecture: architecture,
        ansible_system: os_type,
        ansible_os_family: os_family,
        ansible_distribution: distribution,
    })
}

/// Execute a command in a Docker container
async fn execute_docker_command(
    container: &str,
    command: &[&str],
    timeout_secs: u64,
) -> anyhow::Result<String> {
    let mut cmd = Command::new("docker");
    cmd.arg("exec")
        .arg(container);
    
    for arg in command {
        cmd.arg(arg);
    }
    
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped());
    
    let output = timeout(
        Duration::from_secs(timeout_secs),
        cmd.output()
    )
    .await
    .context("Command timed out")?
    .context("Failed to execute docker command")?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Docker command failed with exit code {}: {}", 
            output.status.code().unwrap_or(-1), 
            stderr));
    }
    
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Check if container is running
async fn check_container_running(container: &str, timeout_secs: u64) -> anyhow::Result<()> {
    let _output = execute_docker_command(
        container,
        &["true"],
        timeout_secs
    ).await?;
    
    Ok(())
}

/// Get OS type
async fn get_os_type(container: &str, timeout_secs: u64) -> anyhow::Result<String> {
    execute_docker_command(
        container,
        &["sh", "-c", "uname -s 2>/dev/null || echo Unknown"],
        timeout_secs
    ).await
}

/// Get hostname
async fn get_hostname(container: &str, timeout_secs: u64) -> anyhow::Result<String> {
    execute_docker_command(
        container,
        &["hostname"],
        timeout_secs
    ).await
}

/// Get kernel info
async fn get_kernel_info(container: &str, timeout_secs: u64) -> anyhow::Result<String> {
    execute_docker_command(
        container,
        &["uname", "-r"],
        timeout_secs
    ).await
}

/// Get CPU info
async fn get_cpu_info(container: &str, timeout_secs: u64) -> anyhow::Result<String> {
    execute_docker_command(
        container,
        &["sh", "-c", "grep -c ^processor /proc/cpuinfo 2>/dev/null || echo 1"],
        timeout_secs
    ).await
}


/// Get architecture
async fn get_architecture(container: &str, timeout_secs: u64) -> anyhow::Result<String> {
    execute_docker_command(
        container,
        &["uname", "-m"],
        timeout_secs
    ).await
}

/// Get distribution name
async fn get_distribution(container: &str, timeout_secs: u64, os_type: &str) -> anyhow::Result<String> {
    if os_type != "Linux" {
        return Ok(os_type.to_string());
    }
    
    // Try various methods to detect distribution
    if let Ok(lsb_release) = execute_docker_command(
        container,
        &["sh", "-c", "lsb_release -si 2>/dev/null"],
        timeout_secs
    ).await {
        if !lsb_release.is_empty() {
            return Ok(lsb_release);
        }
    }
    
    // Try parsing /etc/os-release
    if let Ok(os_release) = execute_docker_command(
        container,
        &["sh", "-c", "grep '^ID=' /etc/os-release 2>/dev/null | cut -d= -f2 | tr -d '\"'"],
        timeout_secs
    ).await {
        if !os_release.is_empty() {
            return Ok(os_release);
        }
    }
    
    // Fallback to checking for specific distribution files
    for (file, distro) in &[
        ("/etc/redhat-release", "RedHat"),
        ("/etc/debian_version", "Debian"),
        ("/etc/alpine-release", "Alpine"),
        ("/etc/arch-release", "Arch"),
    ] {
        if execute_docker_command(
            container,
            &["test", "-f", file],
            timeout_secs
        ).await.is_ok() {
            return Ok(distro.to_string());
        }
    }
    
    Ok("Unknown".to_string())
}


/// Get OS family based on OS type and distribution
fn get_os_family(os_type: &str, distribution: &Option<String>) -> String {
    match os_type.to_lowercase().as_str() {
        "linux" => {
            if let Some(distro) = distribution {
                match distro.to_lowercase().as_str() {
                    "ubuntu" | "debian" | "mint" => "debian".to_string(),
                    "rhel" | "redhat" | "centos" | "fedora" | "rocky" | "almalinux" => "redhat".to_string(),
                    "suse" | "opensuse" => "suse".to_string(),
                    "arch" | "manjaro" => "archlinux".to_string(),
                    "alpine" => "alpine".to_string(),
                    _ => "debian".to_string(), // Default fallback
                }
            } else {
                "debian".to_string() // Default for Linux
            }
        }
        "darwin" => "darwin".to_string(),
        "freebsd" | "openbsd" | "netbsd" => "bsd".to_string(),
        _ => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_get_os_family() {
        assert_eq!(get_os_family("Linux", &Some("ubuntu".to_string())), "debian");
        assert_eq!(get_os_family("Linux", &Some("centos".to_string())), "redhat");
        assert_eq!(get_os_family("Linux", &Some("alpine".to_string())), "alpine");
        assert_eq!(get_os_family("Darwin", &None), "darwin");
        assert_eq!(get_os_family("FreeBSD", &None), "bsd");
        assert_eq!(get_os_family("Windows", &None), "unknown");
    }
}