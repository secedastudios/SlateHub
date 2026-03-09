use slatehub::logging::{format_colored_error, format_database_error, format_http_status, init};

#[test]
fn test_init_does_not_panic() {
    init();
}

#[test]
fn test_format_http_status_2xx() {
    let formatted = format_http_status(200);
    assert!(formatted.contains("200"));
    assert!(formatted.contains("\x1b[32m"));
    assert!(formatted.contains("\x1b[0m"));

    let formatted = format_http_status(201);
    assert!(formatted.contains("201"));
    assert!(formatted.contains("\x1b[32m"));

    let formatted = format_http_status(204);
    assert!(formatted.contains("204"));
    assert!(formatted.contains("\x1b[32m"));
}

#[test]
fn test_format_http_status_3xx() {
    let formatted = format_http_status(301);
    assert!(formatted.contains("301"));
    assert!(formatted.contains("\x1b[33m"));
    assert!(formatted.contains("\x1b[0m"));

    let formatted = format_http_status(302);
    assert!(formatted.contains("302"));
    assert!(formatted.contains("\x1b[33m"));

    let formatted = format_http_status(304);
    assert!(formatted.contains("304"));
    assert!(formatted.contains("\x1b[33m"));
}

#[test]
fn test_format_http_status_4xx() {
    let formatted = format_http_status(400);
    assert!(formatted.contains("400"));
    assert!(formatted.contains("\x1b[38;5;214m"));
    assert!(formatted.contains("\x1b[0m"));

    let formatted = format_http_status(404);
    assert!(formatted.contains("404"));
    assert!(formatted.contains("\x1b[38;5;214m"));

    let formatted = format_http_status(422);
    assert!(formatted.contains("422"));
    assert!(formatted.contains("\x1b[38;5;214m"));
}

#[test]
fn test_format_http_status_5xx() {
    let formatted = format_http_status(500);
    assert!(formatted.contains("500"));
    assert!(formatted.contains("\x1b[31m"));
    assert!(formatted.contains("\x1b[0m"));

    let formatted = format_http_status(502);
    assert!(formatted.contains("502"));
    assert!(formatted.contains("\x1b[31m"));

    let formatted = format_http_status(503);
    assert!(formatted.contains("503"));
    assert!(formatted.contains("\x1b[31m"));
}

#[test]
fn test_format_database_error() {
    let error_message = "Connection failed";
    let formatted = format_database_error(error_message);

    assert!(formatted.contains("Database error:"));
    assert!(formatted.contains(error_message));
    assert!(formatted.contains("\x1b[38;5;215m"));
    assert!(formatted.contains("\x1b[0m"));
}

#[test]
fn test_format_colored_error_database() {
    let error_message = "Table not found";
    let formatted = format_colored_error("database", error_message);

    assert!(formatted.contains("Database error:"));
    assert!(formatted.contains(error_message));
    assert!(formatted.contains("\x1b[38;5;215m"));
    assert!(formatted.contains("\x1b[0m"));
}

#[test]
fn test_format_colored_error_network() {
    let error_message = "Connection timeout";
    let formatted = format_colored_error("http", error_message);

    assert!(formatted.contains("Network error:"));
    assert!(formatted.contains(error_message));
    assert!(formatted.contains("\x1b[38;5;214m"));
    assert!(formatted.contains("\x1b[0m"));
}

#[test]
fn test_format_colored_error_internal() {
    let error_message = "Internal server error";
    let formatted = format_colored_error("internal", error_message);

    assert!(formatted.contains("Internal error:"));
    assert!(formatted.contains(error_message));
    assert!(formatted.contains("\x1b[31m"));
    assert!(formatted.contains("\x1b[0m"));
}

#[test]
fn test_format_colored_error_unknown() {
    let error_message = "Unknown error occurred";
    let formatted = format_colored_error("unknown", error_message);

    assert!(formatted.contains("Error:"));
    assert!(formatted.contains(error_message));
    assert!(formatted.contains("\x1b[38;5;214m"));
    assert!(formatted.contains("\x1b[0m"));
}

#[test]
fn test_edge_cases() {
    let formatted = format_http_status(100);
    assert!(formatted.contains("100"));
    assert!(formatted.contains("\x1b[0m"));

    let formatted = format_http_status(600);
    assert!(formatted.contains("600"));
    assert!(formatted.contains("\x1b[0m"));
}
