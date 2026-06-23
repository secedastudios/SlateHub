//! Unit tests for `slatehub::text` — slug generation and byte formatting.
//! Pure functions; no test DB required.

use slatehub::text::{format_bytes, format_bytes_i64, slugify};

#[test]
fn slugify_collapses_punctuation_runs() {
    assert_eq!(slugify("The Last Deposit!"), "the-last-deposit");
    assert_eq!(slugify("  spaced   out  "), "spaced-out");
    assert_eq!(slugify("Émile's Café #2"), "émile-s-café-2");
}

#[test]
fn slugify_of_only_punctuation_is_empty() {
    assert_eq!(slugify("!!!"), "");
}

#[test]
fn bytes_scale_with_expected_precision() {
    assert_eq!(format_bytes(42), "42 B");
    assert_eq!(format_bytes(2_048), "2 KB");
    assert_eq!(format_bytes(1_572_864), "1.5 MB");
    assert_eq!(format_bytes(3_221_225_472), "3.0 GB");
}

#[test]
fn negative_sizes_clamp_to_zero() {
    assert_eq!(format_bytes_i64(-5), "0 B");
}
