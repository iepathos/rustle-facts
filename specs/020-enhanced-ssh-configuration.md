# Enhanced SSH Configuration, Retry Logic, and Connection Diagnostics

**Spec Number**: 020  
**Feature**: Enhanced SSH Configuration Options  
**Status**: ⬜ Planned  
**Created**: 2025-07-14  

## Feature Summary

This specification outlines enhancements to the SSH connection layer in rustle-facts to provide more robust and configurable remote fact gathering. The improvements include enhanced SSH configuration options, intelligent retry logic with exponential backoff, and detailed connection diagnostics for better troubleshooting and reliability.

### Problem Statement

The current SSH implementation has several limitations:
- Limited SSH configuration options (only SSH config file path)
- No retry mechanism for transient connection failures
- Minimal diagnostic information for connection troubleshooting
- No support for custom SSH key paths or authentication methods
- Limited visibility into connection failure reasons

### Solution Overview

Implement a comprehensive SSH configuration system with:
- Extensive SSH client configuration options
- Intelligent retry logic with exponential backoff
- Detailed connection diagnostics and logging
- Enhanced error categorization and reporting
- Configurable authentication methods and key management

## Goals & Requirements

### Functional Requirements

1. **Enhanced SSH Configuration**
   - Support custom SSH key file paths
   - Configurable SSH port per host or globally
   - SSH client options configuration (cipher, MAC, key exchange)
   - SSH agent forwarding control
   - Custom SSH binary path support

2. **Retry Logic**
   - Configurable retry attempts (0-10)
   - Exponential backoff with jitter
   - Retry only on transient failures (network, timeout)
   - Skip retry on authentication failures
   - Per-host retry state tracking

3. **Connection Diagnostics**
   - Detailed error categorization (network, auth, timeout, command)
   - SSH client version detection
   - Connection timing metrics
   - Host reachability testing
   - SSH fingerprint validation and reporting

### Non-Functional Requirements

- **Performance**: Retry logic should not significantly impact overall execution time
- **Reliability**: Enhanced error handling should improve success rates by 15-30%
- **Observability**: Diagnostic information should enable quick troubleshooting
- **Security**: Enhanced options should maintain or improve security posture
- **Backward Compatibility**: Existing configurations should continue to work

### Success Criteria

- [ ] Successful connection rate improvement for flaky networks
- [ ] Clear diagnostic messages for all failure types
- [ ] Configuration validation and helpful error messages
- [ ] Comprehensive test coverage including edge cases
- [ ] Performance benchmarks showing minimal overhead

## API/Interface Design

### Configuration Structures

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshConfig {
    /// Path to SSH private key file
    pub key_file: Option<PathBuf>,
    
    /// Custom SSH port (overrides host-specific ports)
    pub port: Option<u16>,
    
    /// Path to SSH config file
    pub config_file: Option<PathBuf>,
    
    /// Custom SSH binary path
    pub ssh_binary: Option<PathBuf>,
    
    /// Enable SSH agent forwarding
    pub agent_forwarding: bool,
    
    /// Preferred authentication methods
    pub auth_methods: Vec<SshAuthMethod>,
    
    /// Additional SSH client options
    pub client_options: HashMap<String, String>,
    
    /// Host-specific SSH configurations
    pub host_configs: HashMap<String, HostSshConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostSshConfig {
    pub port: Option<u16>,
    pub key_file: Option<PathBuf>,
    pub user: Option<String>,
    pub proxy_jump: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SshAuthMethod {
    PublicKey,
    Password,
    KeyboardInteractive,
    Agent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u8,
    
    /// Initial delay between retries (milliseconds)
    pub initial_delay: u64,
    
    /// Maximum delay between retries (milliseconds)
    pub max_delay: u64,
    
    /// Exponential backoff multiplier
    pub backoff_multiplier: f64,
    
    /// Add random jitter to retry delays
    pub jitter: bool,
    
    /// Retry on specific error types
    pub retry_on: Vec<RetryableErrorType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetryableErrorType {
    NetworkTimeout,
    ConnectionRefused,
    HostUnreachable,
    TemporaryFailure,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsConfig {
    /// Enable detailed connection diagnostics
    pub enabled: bool,
    
    /// Include timing information
    pub include_timing: bool,
    
    /// Test host reachability before SSH
    pub test_reachability: bool,
    
    /// Validate SSH host fingerprints
    pub validate_fingerprints: bool,
    
    /// Log SSH client commands
    pub log_commands: bool,
}
```

### Enhanced FactsConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactsConfig {
    // Existing fields...
    
    /// SSH-specific configuration
    pub ssh: SshConfig,
    
    /// Retry configuration
    pub retry: RetryConfig,
    
    /// Diagnostics configuration
    pub diagnostics: DiagnosticsConfig,
}
```

### Connection Diagnostics

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionDiagnostics {
    pub host: String,
    pub attempt_number: u8,
    pub start_time: std::time::Instant,
    pub end_time: Option<std::time::Instant>,
    pub error_type: Option<SshErrorType>,
    pub error_message: Option<String>,
    pub ssh_version: Option<String>,
    pub host_reachable: Option<bool>,
    pub fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SshErrorType {
    NetworkError,
    AuthenticationFailed,
    CommandTimeout,
    CommandFailed,
    HostUnreachable,
    PermissionDenied,
    UnknownHost,
    Other(String),
}
```

### Public API Functions

```rust
/// Execute SSH command with enhanced configuration and retry logic
pub async fn execute_ssh_command_enhanced(
    host: &str,
    command: &str,
    config: &FactsConfig,
) -> Result<(String, ConnectionDiagnostics)> {
    // Implementation
}

/// Test SSH connectivity to a host
pub async fn test_ssh_connectivity(
    host: &str,
    config: &FactsConfig,
) -> Result<ConnectionDiagnostics> {
    // Implementation
}

/// Validate SSH configuration
pub fn validate_ssh_config(config: &SshConfig) -> Result<()> {
    // Implementation
}

/// Generate SSH command arguments from configuration
pub fn build_ssh_command_args(
    host: &str,
    command: &str,
    config: &SshConfig,
    host_config: Option<&HostSshConfig>,
) -> Vec<String> {
    // Implementation
}
```

## File and Package Structure

### New Files

```
src/
├── ssh/
│   ├── mod.rs                 # SSH module exports
│   ├── config.rs              # SSH configuration structures
│   ├── client.rs              # Enhanced SSH client implementation
│   ├── retry.rs               # Retry logic implementation
│   ├── diagnostics.rs         # Connection diagnostics
│   └── errors.rs              # SSH-specific error types
├── ssh_facts.rs               # Updated to use new SSH module
└── config.rs                  # Updated with new configuration options
```

### Updated Files

- `src/config.rs` - Add SSH, retry, and diagnostics configuration
- `src/ssh_facts.rs` - Integrate with new SSH module
- `src/error.rs` - Add enhanced SSH error types
- `src/lib.rs` - Export new SSH module

## Implementation Details

### Phase 1: SSH Configuration Enhancement

1. **Create SSH Module Structure**
   ```rust
   // src/ssh/mod.rs
   pub mod config;
   pub mod client;
   pub mod retry;
   pub mod diagnostics;
   pub mod errors;
   
   pub use config::{SshConfig, HostSshConfig, SshAuthMethod};
   pub use client::SshClient;
   pub use retry::{RetryConfig, RetryableErrorType};
   pub use diagnostics::{ConnectionDiagnostics, DiagnosticsConfig};
   pub use errors::SshErrorType;
   ```

2. **Implement Configuration Structures**
   - Define all configuration structs with serde support
   - Add validation methods for configuration
   - Implement default values following security best practices

3. **Enhanced SSH Command Builder**
   ```rust
   impl SshClient {
       fn build_command_args(&self, host: &str, command: &str) -> Vec<String> {
           let mut args = vec!["ssh".to_string()];
           
           // Add timeout
           args.extend(["-o".to_string(), format!("ConnectTimeout={}", self.config.timeout)]);
           
           // Add custom options
           for (key, value) in &self.config.ssh.client_options {
               args.extend(["-o".to_string(), format!("{}={}", key, value)]);
           }
           
           // Add key file if specified
           if let Some(key_file) = &self.config.ssh.key_file {
               args.extend(["-i".to_string(), key_file.to_string_lossy().to_string()]);
           }
           
           // Add host-specific configurations
           if let Some(host_config) = self.config.ssh.host_configs.get(host) {
               if let Some(port) = host_config.port {
                   args.extend(["-p".to_string(), port.to_string()]);
               }
           }
           
           args.push(host.to_string());
           args.push(command.to_string());
           args
       }
   }
   ```

### Phase 2: Retry Logic Implementation

1. **Retry Strategy**
   ```rust
   pub struct RetryStrategy {
       config: RetryConfig,
       attempt: u8,
       last_error: Option<SshErrorType>,
   }
   
   impl RetryStrategy {
       pub fn should_retry(&self, error: &SshErrorType) -> bool {
           if self.attempt >= self.config.max_attempts {
               return false;
           }
           
           match error {
               SshErrorType::AuthenticationFailed => false,
               SshErrorType::PermissionDenied => false,
               SshErrorType::NetworkError => true,
               SshErrorType::CommandTimeout => true,
               SshErrorType::HostUnreachable => true,
               _ => self.config.retry_on.contains(&error.into()),
           }
       }
       
       pub async fn delay(&mut self) -> Duration {
           let base_delay = self.config.initial_delay as f64;
           let multiplier = self.config.backoff_multiplier.powi(self.attempt as i32);
           let delay = (base_delay * multiplier) as u64;
           let delay = delay.min(self.config.max_delay);
           
           let final_delay = if self.config.jitter {
               let jitter = thread_rng().gen_range(0.8..1.2);
               (delay as f64 * jitter) as u64
           } else {
               delay
           };
           
           self.attempt += 1;
           Duration::from_millis(final_delay)
       }
   }
   ```

2. **Main Retry Loop**
   ```rust
   pub async fn execute_with_retry(
       &self,
       host: &str,
       command: &str,
   ) -> Result<(String, ConnectionDiagnostics)> {
       let mut retry_strategy = RetryStrategy::new(self.config.retry.clone());
       let mut diagnostics = ConnectionDiagnostics::new(host);
       
       loop {
           diagnostics.start_attempt();
           
           match self.execute_single_attempt(host, command).await {
               Ok(output) => {
                   diagnostics.record_success();
                   return Ok((output, diagnostics));
               }
               Err(error) => {
                   diagnostics.record_error(&error);
                   
                   if !retry_strategy.should_retry(&error.error_type()) {
                       return Err(error);
                   }
                   
                   let delay = retry_strategy.delay().await;
                   warn!("SSH attempt {} failed for {}, retrying in {:?}: {}", 
                         retry_strategy.attempt, host, delay, error);
                   
                   tokio::time::sleep(delay).await;
               }
           }
       }
   }
   ```

### Phase 3: Connection Diagnostics

1. **Diagnostic Collection**
   ```rust
   impl ConnectionDiagnostics {
       pub fn new(host: &str) -> Self {
           Self {
               host: host.to_string(),
               attempt_number: 1,
               start_time: Instant::now(),
               end_time: None,
               error_type: None,
               error_message: None,
               ssh_version: None,
               host_reachable: None,
               fingerprint: None,
           }
       }
       
       pub async fn test_reachability(&mut self) -> Result<()> {
           let output = Command::new("ping")
               .arg("-c")
               .arg("1")
               .arg("-W")
               .arg("1")
               .arg(&self.host)
               .output()
               .await?;
           
           self.host_reachable = Some(output.status.success());
           Ok(())
       }
       
       pub async fn detect_ssh_version(&mut self) -> Result<()> {
           let output = Command::new("ssh")
               .arg("-V")
               .output()
               .await?;
           
           if output.status.success() {
               let version = String::from_utf8_lossy(&output.stderr);
               self.ssh_version = Some(version.trim().to_string());
           }
           
           Ok(())
       }
   }
   ```

2. **Error Categorization**
   ```rust
   impl From<std::io::Error> for SshErrorType {
       fn from(error: std::io::Error) -> Self {
           match error.kind() {
               std::io::ErrorKind::TimedOut => SshErrorType::CommandTimeout,
               std::io::ErrorKind::ConnectionRefused => SshErrorType::NetworkError,
               std::io::ErrorKind::PermissionDenied => SshErrorType::PermissionDenied,
               _ => SshErrorType::Other(error.to_string()),
           }
       }
   }
   
   pub fn categorize_ssh_error(stderr: &str, exit_code: i32) -> SshErrorType {
       let stderr_lower = stderr.to_lowercase();
       
       if stderr_lower.contains("permission denied") {
           SshErrorType::AuthenticationFailed
       } else if stderr_lower.contains("connection refused") {
           SshErrorType::NetworkError
       } else if stderr_lower.contains("no route to host") {
           SshErrorType::HostUnreachable
       } else if stderr_lower.contains("host key verification failed") {
           SshErrorType::UnknownHost
       } else if stderr_lower.contains("operation timed out") {
           SshErrorType::CommandTimeout
       } else {
           SshErrorType::Other(format!("Exit code {}: {}", exit_code, stderr))
       }
   }
   ```

## Testing Strategy

### Unit Tests

1. **Configuration Tests** (`src/ssh/config.rs`)
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       
       #[test]
       fn test_ssh_config_defaults() {
           let config = SshConfig::default();
           assert!(!config.agent_forwarding);
           assert!(config.host_configs.is_empty());
       }
       
       #[test]
       fn test_ssh_config_validation() {
           let mut config = SshConfig::default();
           config.key_file = Some(PathBuf::from("/nonexistent/key"));
           assert!(validate_ssh_config(&config).is_err());
       }
       
       #[test]
       fn test_host_specific_config() {
           let mut config = SshConfig::default();
           config.host_configs.insert(
               "test.host".to_string(),
               HostSshConfig {
                   port: Some(2222),
                   key_file: None,
                   user: Some("testuser".to_string()),
                   proxy_jump: None,
               }
           );
           
           let host_config = config.host_configs.get("test.host").unwrap();
           assert_eq!(host_config.port, Some(2222));
       }
   }
   ```

2. **Retry Logic Tests** (`src/ssh/retry.rs`)
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       
       #[test]
       fn test_retry_strategy_should_retry() {
           let config = RetryConfig {
               max_attempts: 3,
               retry_on: vec![RetryableErrorType::NetworkTimeout],
               ..Default::default()
           };
           
           let mut strategy = RetryStrategy::new(config);
           assert!(strategy.should_retry(&SshErrorType::NetworkError));
           assert!(!strategy.should_retry(&SshErrorType::AuthenticationFailed));
       }
       
       #[tokio::test]
       async fn test_exponential_backoff() {
           let config = RetryConfig {
               initial_delay: 100,
               backoff_multiplier: 2.0,
               max_delay: 1000,
               jitter: false,
               ..Default::default()
           };
           
           let mut strategy = RetryStrategy::new(config);
           let delay1 = strategy.delay().await;
           let delay2 = strategy.delay().await;
           
           assert_eq!(delay1, Duration::from_millis(100));
           assert_eq!(delay2, Duration::from_millis(200));
       }
   }
   ```

3. **Diagnostics Tests** (`src/ssh/diagnostics.rs`)
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       
       #[test]
       fn test_error_categorization() {
           assert_eq!(
               categorize_ssh_error("Permission denied (publickey)", 255),
               SshErrorType::AuthenticationFailed
           );
           
           assert_eq!(
               categorize_ssh_error("Connection refused", 255),
               SshErrorType::NetworkError
           );
       }
       
       #[test]
       fn test_diagnostics_timing() {
           let mut diag = ConnectionDiagnostics::new("test.host");
           diag.start_attempt();
           std::thread::sleep(Duration::from_millis(10));
           diag.record_success();
           
           assert!(diag.end_time.is_some());
           assert!(diag.duration().unwrap() >= Duration::from_millis(10));
       }
   }
   ```

### Integration Tests

1. **Full SSH Flow Tests** (`tests/ssh_integration.rs`)
   ```rust
   #[tokio::test]
   async fn test_ssh_with_retry_success_after_failure() {
       let config = FactsConfig {
           retry: RetryConfig {
               max_attempts: 3,
               initial_delay: 10,
               ..Default::default()
           },
           ..Default::default()
       };
       
       // Mock SSH client that fails twice then succeeds
       let client = MockSshClient::new()
           .expect_failure(SshErrorType::NetworkError)
           .expect_failure(SshErrorType::NetworkError)
           .expect_success("ARCH=x86_64\nSYSTEM=Linux");
       
       let result = client.execute_with_retry("test.host", "echo test").await;
       assert!(result.is_ok());
   }
   
   #[tokio::test]
   async fn test_ssh_authentication_failure_no_retry() {
       let config = FactsConfig {
           retry: RetryConfig {
               max_attempts: 3,
               ..Default::default()
           },
           ..Default::default()
       };
       
       let client = MockSshClient::new()
           .expect_failure(SshErrorType::AuthenticationFailed);
       
       let result = client.execute_with_retry("test.host", "echo test").await;
       assert!(result.is_err());
       // Should only attempt once
       assert_eq!(client.attempt_count(), 1);
   }
   ```

2. **Configuration Integration Tests**
   ```rust
   #[tokio::test]
   async fn test_host_specific_ssh_config() {
       let mut config = FactsConfig::default();
       config.ssh.host_configs.insert(
           "special.host".to_string(),
           HostSshConfig {
               port: Some(2222),
               user: Some("special_user".to_string()),
               ..Default::default()
           }
       );
       
       let client = SshClient::new(config);
       let args = client.build_command_args("special.host", "echo test");
       
       assert!(args.contains(&"-p".to_string()));
       assert!(args.contains(&"2222".to_string()));
       assert!(args.contains(&"special_user@special.host".to_string()));
   }
   ```

### Test File Structure

```
tests/
├── ssh_integration.rs         # Integration tests for SSH functionality
├── ssh_retry_tests.rs         # Retry logic integration tests
├── ssh_diagnostics_tests.rs   # Diagnostics integration tests
└── fixtures/
    ├── ssh_configs/           # Test SSH configurations
    └── mock_responses/        # Mock SSH command responses
```

## Edge Cases & Error Handling

### Edge Cases

1. **Network Connectivity Issues**
   - Intermittent network failures during fact gathering
   - DNS resolution failures
   - Firewall blocking SSH connections
   - Network partitions affecting subset of hosts

2. **SSH Authentication Edge Cases**
   - SSH keys with passphrases
   - SSH agent not running
   - Host key changes (MITM protection)
   - Multiple authentication methods required

3. **Configuration Edge Cases**
   - SSH config file parsing errors
   - Invalid SSH key file permissions
   - Conflicting host-specific and global configurations
   - Missing SSH binary on system

4. **Resource Constraints**
   - High connection count hitting system limits
   - Memory pressure during concurrent connections
   - SSH connection pooling limits
   - Timeout during large-scale deployments

### Error Handling Patterns

1. **Graceful Degradation**
   ```rust
   impl SshClient {
       async fn execute_with_fallback(&self, host: &str, command: &str) -> Result<String> {
           match self.execute_with_retry(host, command).await {
               Ok((output, _)) => Ok(output),
               Err(ssh_error) => {
                   warn!("SSH failed for {}, attempting fallback: {}", host, ssh_error);
                   
                   if ArchitectureFacts::should_use_local_detection(host, &HashMap::new()) {
                       info!("Using local detection fallback for {}", host);
                       Ok(self.execute_local_detection())
                   } else {
                       warn!("Using fallback facts for {}", host);
                       Ok(ArchitectureFacts::fallback().to_fact_output())
                   }
               }
           }
       }
   }
   ```

2. **Resource Cleanup**
   ```rust
   impl Drop for SshClient {
       fn drop(&mut self) {
           // Ensure all SSH processes are terminated
           if let Some(child) = self.current_process.take() {
               let _ = child.kill();
           }
       }
   }
   ```

3. **Configuration Validation**
   ```rust
   pub fn validate_ssh_config(config: &SshConfig) -> Result<Vec<String>> {
       let mut warnings = Vec::new();
       
       if let Some(key_file) = &config.key_file {
           if !key_file.exists() {
               return Err(FactsError::ConfigError(
                   format!("SSH key file does not exist: {:?}", key_file)
               ));
           }
           
           let metadata = std::fs::metadata(key_file)?;
           let permissions = metadata.permissions();
           if permissions.mode() & 0o077 != 0 {
               warnings.push(format!(
                   "SSH key file {:?} has overly permissive permissions", 
                   key_file
               ));
           }
       }
       
       Ok(warnings)
   }
   ```

## Dependencies

### External Crates

```toml
[dependencies]
# Existing dependencies...

# For SSH fingerprint handling
ssh2 = { version = "0.9", optional = true }

# For network connectivity testing  
ping = "0.5"

# For random jitter in retry delays
rand = "0.8"

# For SSH key validation
ssh-key = "0.6"

[features]
default = ["ssh-validation"]
ssh-validation = ["ssh2", "ssh-key"]
```

### Internal Dependencies

- `crate::config` - Enhanced configuration structures
- `crate::error` - Extended error types
- `crate::cache` - SSH fingerprint caching
- `crate::types` - Architecture facts and diagnostics

### Platform-Specific Considerations

1. **Windows Support**
   ```rust
   #[cfg(windows)]
   fn get_default_ssh_binary() -> PathBuf {
       // Check for OpenSSH in Windows 10+
       PathBuf::from("C:\\Windows\\System32\\OpenSSH\\ssh.exe")
   }
   
   #[cfg(unix)]
   fn get_default_ssh_binary() -> PathBuf {
       PathBuf::from("/usr/bin/ssh")
   }
   ```

2. **macOS SSH Agent Integration**
   ```rust
   #[cfg(target_os = "macos")]
   fn configure_ssh_agent_options(args: &mut Vec<String>) {
       args.extend([
           "-o".to_string(),
           "UseKeychain=yes".to_string()
       ]);
   }
   ```

## Configuration

### CLI Arguments

```rust
#[derive(Debug, Clone, Parser)]
pub struct CliArgs {
    // Existing arguments...
    
    #[arg(long, value_name = "PATH", help = "Path to SSH private key")]
    pub ssh_key: Option<PathBuf>,
    
    #[arg(long, value_name = "PORT", help = "Default SSH port")]
    pub ssh_port: Option<u16>,
    
    #[arg(long, value_name = "COUNT", default_value = "3", help = "SSH retry attempts")]
    pub ssh_retries: u8,
    
    #[arg(long, help = "Enable SSH connection diagnostics")]
    pub ssh_diagnostics: bool,
    
    #[arg(long, value_name = "MS", default_value = "1000", help = "Initial retry delay")]
    pub retry_delay: u64,
    
    #[arg(long, help = "Test host reachability before SSH")]
    pub test_reachability: bool,
}
```

### Environment Variables

```rust
impl FactsConfig {
    pub fn from_env() -> Self {
        let mut config = Self::default();
        
        // Existing environment variables...
        
        if let Ok(ssh_key) = std::env::var("RUSTLE_FACTS_SSH_KEY") {
            config.ssh.key_file = Some(PathBuf::from(ssh_key));
        }
        
        if let Ok(ssh_port) = std::env::var("RUSTLE_FACTS_SSH_PORT") {
            if let Ok(port) = ssh_port.parse() {
                config.ssh.port = Some(port);
            }
        }
        
        if let Ok(retries) = std::env::var("RUSTLE_FACTS_SSH_RETRIES") {
            if let Ok(retry_count) = retries.parse() {
                config.retry.max_attempts = retry_count;
            }
        }
        
        config
    }
}
```

### Default Configuration

```rust
impl Default for SshConfig {
    fn default() -> Self {
        Self {
            key_file: None,
            port: None,
            config_file: None,
            ssh_binary: None,
            agent_forwarding: false,
            auth_methods: vec![SshAuthMethod::PublicKey, SshAuthMethod::Agent],
            client_options: HashMap::from([
                ("StrictHostKeyChecking".to_string(), "no".to_string()),
                ("UserKnownHostsFile".to_string(), "/dev/null".to_string()),
                ("BatchMode".to_string(), "yes".to_string()),
            ]),
            host_configs: HashMap::new(),
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: 1000,
            max_delay: 30000,
            backoff_multiplier: 2.0,
            jitter: true,
            retry_on: vec![
                RetryableErrorType::NetworkTimeout,
                RetryableErrorType::ConnectionRefused,
                RetryableErrorType::TemporaryFailure,
            ],
        }
    }
}
```

## Documentation

### Public API Documentation

```rust
/// Enhanced SSH client with retry logic and diagnostics
/// 
/// This client provides robust SSH connectivity with configurable retry
/// behavior and detailed connection diagnostics for troubleshooting.
/// 
/// # Examples
/// 
/// ```rust
/// use rustle_facts::{SshClient, FactsConfig};
/// 
/// let config = FactsConfig {
///     ssh: SshConfig {
///         key_file: Some("/path/to/key".into()),
///         ..Default::default()
///     },
///     retry: RetryConfig {
///         max_attempts: 5,
///         ..Default::default()
///     },
///     ..Default::default()
/// };
/// 
/// let client = SshClient::new(config);
/// let (output, diagnostics) = client
///     .execute_with_retry("user@host", "uname -a")
///     .await?;
/// 
/// println!("Command output: {}", output);
/// println!("Connection took: {:?}", diagnostics.duration());
/// ```
pub struct SshClient {
    config: FactsConfig,
}
```

### Configuration Examples

```rust
/// # SSH Configuration Examples
/// 
/// ## Basic SSH key configuration
/// ```toml
/// [ssh]
/// key_file = "/home/user/.ssh/id_rsa"
/// port = 22
/// agent_forwarding = false
/// 
/// [retry]
/// max_attempts = 3
/// initial_delay = 1000
/// jitter = true
/// ```
/// 
/// ## Host-specific configuration
/// ```toml
/// [ssh.host_configs.production]
/// port = 2222
/// key_file = "/home/user/.ssh/prod_key"
/// user = "deploy"
/// 
/// [ssh.host_configs.staging] 
/// port = 22
/// proxy_jump = "bastion.example.com"
/// ```
```

### Error Handling Guide

```rust
/// # SSH Error Handling
/// 
/// The SSH client categorizes errors to enable appropriate handling:
/// 
/// - **Authentication Errors**: No retry, requires user intervention
/// - **Network Errors**: Retried with exponential backoff
/// - **Command Errors**: May be retried depending on configuration
/// 
/// ## Example Error Handling
/// 
/// ```rust
/// match client.execute_with_retry(host, command).await {
///     Ok((output, diagnostics)) => {
///         if diagnostics.attempt_number > 1 {
///             warn!("Command succeeded after {} attempts", diagnostics.attempt_number);
///         }
///         Ok(output)
///     }
///     Err(error) => {
///         match error.error_type() {
///             SshErrorType::AuthenticationFailed => {
///                 error!("Authentication failed for {}: check SSH keys", host);
///             }
///             SshErrorType::NetworkError => {
///                 error!("Network error for {}: check connectivity", host);
///             }
///             _ => {
///                 error!("SSH error for {}: {}", host, error);
///             }
///         }
///         Err(error)
///     }
/// }
/// ```
```

## Implementation Timeline

### Phase 1: Core Infrastructure (Week 1)
- [ ] Create SSH module structure
- [ ] Implement configuration structures
- [ ] Add configuration validation
- [ ] Update FactsConfig integration

### Phase 2: Enhanced SSH Client (Week 2)
- [ ] Implement enhanced command builder
- [ ] Add host-specific configuration support
- [ ] Integrate with existing ssh_facts module
- [ ] Add comprehensive unit tests

### Phase 3: Retry Logic (Week 3)
- [ ] Implement retry strategy
- [ ] Add exponential backoff with jitter
- [ ] Error categorization system
- [ ] Integration testing with mock failures

### Phase 4: Diagnostics & Observability (Week 4)
- [ ] Connection diagnostics implementation
- [ ] SSH version detection
- [ ] Host reachability testing
- [ ] Enhanced logging and metrics

### Phase 5: Testing & Documentation (Week 5)
- [ ] Comprehensive integration tests
- [ ] Performance benchmarking
- [ ] Documentation and examples
- [ ] Backward compatibility verification

## Success Metrics

- **Reliability**: 95% connection success rate in test environments
- **Performance**: < 5% overhead from retry logic and diagnostics
- **Observability**: Clear diagnosis for 90% of connection failures
- **Usability**: Configuration validation catches 95% of user errors
- **Compatibility**: 100% backward compatibility with existing configurations