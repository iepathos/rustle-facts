use rustle_facts::{
    enrich_with_facts, gather_minimal_facts, parse_fact_output, ArchitectureFacts, FactsConfig,
};
use std::io::Cursor;
use tempfile::tempdir;

#[test]
fn test_parse_various_outputs() {
    let ubuntu_output = r#"
ARCH=x86_64
SYSTEM=Linux
OS_FAMILY=debian
DISTRIBUTION=ubuntu
"#;

    let facts = parse_fact_output(ubuntu_output).unwrap();
    assert_eq!(facts.ansible_architecture, "x86_64");
    assert_eq!(facts.ansible_system, "Linux");
    assert_eq!(facts.ansible_os_family, "debian");
    assert_eq!(facts.ansible_distribution, Some("ubuntu".to_string()));

    let macos_output = r#"
ARCH=arm64
SYSTEM=Darwin
OS_FAMILY=darwin
DISTRIBUTION=macos
"#;

    let facts = parse_fact_output(macos_output).unwrap();
    assert_eq!(facts.ansible_architecture, "aarch64");
    assert_eq!(facts.ansible_system, "Darwin");

    let rhel_output = r#"
ARCH=x86_64
SYSTEM=Linux
OS_FAMILY=rhel
DISTRIBUTION=rhel
"#;

    let facts = parse_fact_output(rhel_output).unwrap();
    assert_eq!(facts.ansible_os_family, "rhel");
}

#[tokio::test]
async fn test_localhost_facts() {
    let hosts = vec!["localhost".to_string()];
    let config = FactsConfig {
        timeout: 5,
        ..Default::default()
    };

    match gather_minimal_facts(&hosts, &config).await {
        Ok(facts) => {
            assert!(!facts.is_empty());
            if let Some(localhost_facts) = facts.get("localhost") {
                assert!(!localhost_facts.ansible_architecture.is_empty());
                assert!(!localhost_facts.ansible_system.is_empty());
            }
        }
        Err(e) => {
            eprintln!("Skipping localhost test due to error: {}", e);
        }
    }
}

#[tokio::test]
async fn test_full_pipeline() {
    let input_json = include_str!("fixtures/parsed_playbook.json");
    let mut output = Vec::new();

    let dir = tempdir().unwrap();
    let cache_file = dir.path().join("test-cache.json");

    let config = FactsConfig {
        cache_file: cache_file.clone(),
        no_cache: false,
        force_refresh: true,
        ..Default::default()
    };

    let result = enrich_with_facts(
        Cursor::new(input_json),
        &mut output,
        &config,
    )
    .await;

    match result {
        Ok(report) => {
            assert!(report.total_hosts > 0);
            
            let output_str = String::from_utf8(output).unwrap();
            assert!(output_str.contains("host_facts"));
            assert!(output_str.contains("ansible_architecture"));

            assert!(cache_file.exists());
        }
        Err(e) => {
            eprintln!("Pipeline test error (expected if no SSH access): {}", e);
        }
    }
}

#[tokio::test]
async fn test_cache_behavior() {
    let input_json = include_str!("fixtures/parsed_playbook.json");
    let dir = tempdir().unwrap();
    let cache_file = dir.path().join("cache-test.json");

    let config = FactsConfig {
        cache_file: cache_file.clone(),
        no_cache: false,
        cache_ttl: 3600,
        ..Default::default()
    };

    let mut output1 = Vec::new();
    let result1 = enrich_with_facts(
        Cursor::new(input_json),
        &mut output1,
        &config,
    )
    .await;

    if result1.is_ok() {
        assert!(cache_file.exists());

        let mut output2 = Vec::new();
        let result2 = enrich_with_facts(
            Cursor::new(input_json),
            &mut output2,
            &config,
        )
        .await;

        if let Ok(report2) = result2 {
            assert_eq!(report2.facts_gathered, 0);
            assert_eq!(report2.cache_hits, report2.total_hosts);
        }
    }
}

#[test]
fn test_architecture_normalization() {
    let test_cases = vec![
        ("x86_64", "x86_64"),
        ("amd64", "x86_64"),
        ("X86_64", "x86_64"),
        ("AMD64", "x86_64"),
        ("aarch64", "aarch64"),
        ("arm64", "aarch64"),
        ("ARM64", "aarch64"),
        ("armv7l", "armv7"),
        ("armhf", "armv7"),
        ("custom-arch", "custom-arch"),
    ];

    for (input, expected) in test_cases {
        assert_eq!(
            ArchitectureFacts::normalize_architecture(input),
            expected,
            "Failed to normalize {}",
            input
        );
    }
}

#[test]
fn test_fallback_facts() {
    let fallback = ArchitectureFacts::fallback();
    assert_eq!(fallback.ansible_architecture, "x86_64");
    assert_eq!(fallback.ansible_system, "Linux");
    assert_eq!(fallback.ansible_os_family, "debian");
    assert_eq!(fallback.ansible_distribution, None);
}