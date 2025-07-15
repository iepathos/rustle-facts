use rustle_facts::{enrich_with_facts, FactsConfig};
use std::io::Cursor;
use std::process::Command;
use std::thread;
use std::time::Duration;

const CONTAINER_NAME: &str = "rustle-test-container";
const DOCKER_IMAGE: &str = "ubuntu:24.04";

#[tokio::test]
async fn test_docker_facts_gathering() {
    // Check if Docker is available
    if !is_docker_available() {
        println!("Docker not available, skipping test");
        return;
    }

    // Ensure container is cleaned up before test
    cleanup_container();

    // Start Ubuntu 24 container
    let container_started = start_test_container();
    assert!(container_started, "Failed to start test container");

    // Give container time to fully start
    thread::sleep(Duration::from_secs(2));

    // Read the test fixture
    let fixture_data = std::fs::read_to_string("tests/fixtures/docker_test_playbook.json")
        .expect("Failed to read fixture file");

    let mut output = Vec::new();
    let config = FactsConfig {
        no_cache: true,
        force_refresh: true,
        ..Default::default()
    };

    let input = Cursor::new(fixture_data);
    let result = enrich_with_facts(input, &mut output, &config).await;

    // Check results
    match result {
        Ok(report) => {
            assert!(report.facts_gathered > 0, "Should have gathered facts");

            // Parse output and verify Docker host facts
            let output_str = String::from_utf8(output).expect("Invalid UTF-8");
            let enriched: serde_json::Value =
                serde_json::from_str(&output_str).expect("Failed to parse enriched output");

            // Check dockerhost facts
            let docker_facts = &enriched["inventory"]["host_facts"]["dockerhost"];
            assert!(
                !docker_facts.is_null(),
                "Docker host facts should be present"
            );

            // Debug output
            eprintln!(
                "Docker facts: {}",
                serde_json::to_string_pretty(docker_facts).unwrap()
            );

            assert_eq!(docker_facts["ansible_system"], "Linux");
            assert_eq!(docker_facts["ansible_os_family"], "debian");

            // Check distribution
            assert!(
                !docker_facts["ansible_distribution"].is_null(),
                "ansible_distribution should not be null for Ubuntu container"
            );
            assert_eq!(docker_facts["ansible_distribution"], "ubuntu");
        }
        Err(e) => {
            cleanup_container();
            panic!("Failed to enrich with facts: {}", e);
        }
    }

    // Clean up container
    cleanup_container();
}

fn is_docker_available() -> bool {
    Command::new("docker")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn start_test_container() -> bool {
    // Pull the image first
    let pull_result = Command::new("docker")
        .args(&["pull", DOCKER_IMAGE])
        .output()
        .expect("Failed to execute docker pull");

    if !pull_result.status.success() {
        eprintln!(
            "Failed to pull Docker image: {}",
            String::from_utf8_lossy(&pull_result.stderr)
        );
        return false;
    }

    // Run the container
    let run_result = Command::new("docker")
        .args(&[
            "run",
            "-d",
            "--name",
            CONTAINER_NAME,
            "--rm",
            DOCKER_IMAGE,
            "sleep",
            "infinity",
        ])
        .output()
        .expect("Failed to execute docker run");

    if !run_result.status.success() {
        eprintln!(
            "Failed to start container: {}",
            String::from_utf8_lossy(&run_result.stderr)
        );
        return false;
    }

    true
}

fn cleanup_container() {
    // Stop and remove container if it exists
    let _ = Command::new("docker")
        .args(&["stop", CONTAINER_NAME])
        .output();

    // Give it a moment to stop
    thread::sleep(Duration::from_millis(500));

    // Force remove just in case
    let _ = Command::new("docker")
        .args(&["rm", "-f", CONTAINER_NAME])
        .output();
}

#[test]
fn test_docker_command_available() {
    if is_docker_available() {
        println!("Docker is available");
    } else {
        println!("Docker is not available - Docker tests will be skipped");
    }
}
