use rustle_facts::{enrich_with_facts, EnrichedPlaybook, FactsConfig};
use std::io::Cursor;
use tempfile::tempdir;

#[tokio::test]
async fn test_real_rustle_parse_output() {
    let input_json = include_str!("fixtures/file_operations.json");
    let mut output = Vec::new();

    let dir = tempdir().unwrap();
    let cache_file = dir.path().join("test-cache.json");

    let config = FactsConfig {
        cache_file: cache_file.clone(),
        no_cache: false,
        force_refresh: true,
        ..Default::default()
    };

    let result = enrich_with_facts(Cursor::new(input_json), &mut output, &config).await;

    match result {
        Ok(report) => {
            println!("Enrichment report: {report:?}");

            let output_str = String::from_utf8(output).unwrap();

            let enriched: EnrichedPlaybook = serde_json::from_str(&output_str).unwrap();

            assert_eq!(
                enriched.playbook.metadata.file_path,
                Some("tests/fixtures/playbooks/file_operations_playbook.yml".to_string())
            );

            assert_eq!(enriched.playbook.plays.len(), 1);
            assert_eq!(enriched.playbook.plays[0].hosts, "all");
            assert_eq!(enriched.playbook.plays[0].tasks.len(), 5);

            assert_eq!(enriched.playbook.plays[0].tasks[0].module, "file");
            assert_eq!(enriched.playbook.plays[0].tasks[1].module, "file");
            assert_eq!(enriched.playbook.plays[0].tasks[2].module, "copy");

            assert_eq!(
                enriched.playbook.plays[0].tasks[4].when,
                Some("ansible_system != \"Windows\"".to_string())
            );

            if !enriched.inventory.host_facts.is_empty() {
                for (host, facts) in &enriched.inventory.host_facts {
                    println!("Host: {host}, Facts: {facts:?}");
                    assert!(!facts.ansible_architecture.is_empty());
                    assert!(!facts.ansible_system.is_empty());
                }
            }
        }
        Err(e) => {
            eprintln!("Expected error if no hosts in inventory: {e}");
            assert!(
                e.to_string().contains("No hosts found")
                    || e.to_string().contains("Invalid inventory")
            );
        }
    }
}

#[test]
fn test_parse_real_playbook_structure() {
    let input_json = include_str!("fixtures/file_operations.json");
    let parsed: serde_json::Value = serde_json::from_str(input_json).unwrap();

    assert!(parsed["metadata"]["file_path"].is_string());
    assert!(parsed["plays"].is_array());
    assert_eq!(parsed["plays"].as_array().unwrap().len(), 1);

    let first_play = &parsed["plays"][0];
    assert_eq!(first_play["hosts"], "all");
    assert!(first_play["tasks"].is_array());
    assert_eq!(first_play["tasks"].as_array().unwrap().len(), 5);

    assert_eq!(parsed["facts_required"], false);
}
