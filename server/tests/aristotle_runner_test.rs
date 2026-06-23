//! End-to-end smoke tests for the `aristotle_runner` wrapper.
//!
//! These exercise the full path: bytes in → parse → tier 0–2 breakdown →
//! `BreakdownOutput` struct out. No DB, no HTTP — pure library + wrapper.
//!
//! The wrapper is concurrency-capped (semaphore-gated); the concurrency
//! test confirms that many simultaneous calls serialize without errors.

use slatehub::services::aristotle_runner::{BreakdownOutput, run_breakdown};

const SAMPLE_FOUNTAIN: &str = "\
Title: Test Script
Author: Test Author

INT. APARTMENT - NIGHT

Rain hammers the windows. MAYA, 30s, paces.

MAYA
(whispering)
Where are you?

She grabs a knife.

VIKTOR (O.S.)
Right behind you.

CUT TO:

EXT. ALLEY - CONTINUOUS

A crowd gathers.
";

#[tokio::test]
async fn happy_path_returns_scenes() {
    let bytes = SAMPLE_FOUNTAIN.as_bytes().to_vec();
    let output: BreakdownOutput = run_breakdown(
        "test_script.fountain".to_string(),
        bytes,
        "test-job-1".to_string(),
    )
    .await
    .expect("breakdown succeeded");

    assert!(
        !output.scenes.is_empty(),
        "expected at least one scene from a 2-scene Fountain sample"
    );
    assert_eq!(output.scenes.len(), 2, "sample has 2 scenes");
    assert!(
        output.scenes[0].scene_heading.contains("APARTMENT"),
        "first scene heading should mention APARTMENT — got {:?}",
        output.scenes[0].scene_heading
    );
    assert!(
        output.scenes[1].scene_heading.contains("ALLEY"),
        "second scene heading should mention ALLEY"
    );
}

#[tokio::test]
async fn happy_path_detects_speaking_cast() {
    let bytes = SAMPLE_FOUNTAIN.as_bytes().to_vec();
    let output = run_breakdown(
        "test_script.fountain".to_string(),
        bytes,
        "test-job-2".to_string(),
    )
    .await
    .expect("breakdown succeeded");

    // Tier 1 structural extracts character cues into speaking_cast.
    let scene1 = &output.scenes[0];
    assert!(
        scene1.speaking_cast.iter().any(|c| c.contains("MAYA")),
        "Maya should be in scene 1 speaking cast — got {:?}",
        scene1.speaking_cast
    );
}

#[tokio::test]
async fn corrupt_bytes_return_useful_error() {
    // Bogus PDF — empty/garbage bytes claimed as PDF.
    let bytes = b"not a real pdf at all".to_vec();
    let result = run_breakdown("garbage.pdf".to_string(), bytes, "test-bad".to_string()).await;

    assert!(
        result.is_err(),
        "garbage PDF bytes should not produce a successful breakdown"
    );
    let err_str = format!("{:?}", result.err().unwrap());
    assert!(
        err_str.to_lowercase().contains("parse") || err_str.to_lowercase().contains("external"),
        "error should reference parse failure — got {err_str}"
    );
}

#[tokio::test]
async fn unsupported_extension_returns_useful_error() {
    let bytes = b"some text".to_vec();
    let result = run_breakdown(
        "screenplay.docx".to_string(),
        bytes,
        "test-unsupported".to_string(),
    )
    .await;

    assert!(result.is_err(), "unsupported format should error");
}

#[tokio::test]
async fn concurrent_jobs_serialize_through_semaphore() {
    // Spawn more concurrent jobs than the semaphore permits (4).
    // All should succeed without errors — the semaphore just serializes
    // the excess. The exact timing isn't asserted; only correctness.
    let mut handles = Vec::new();
    for i in 0..8 {
        let bytes = SAMPLE_FOUNTAIN.as_bytes().to_vec();
        handles.push(tokio::spawn(async move {
            run_breakdown(
                "test_script.fountain".to_string(),
                bytes,
                format!("test-concurrent-{i}"),
            )
            .await
        }));
    }
    for h in handles {
        let result = h.await.expect("task joined");
        assert!(
            result.is_ok(),
            "concurrent breakdown #{} failed: {:?}",
            "?",
            result.err()
        );
    }
}
