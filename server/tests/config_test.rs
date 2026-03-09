use slatehub::config::{DatabaseConfig, ServerConfig};

#[test]
fn test_database_connection_url() {
    let config = DatabaseConfig {
        host: "localhost".to_string(),
        port: 8000,
        username: "root".to_string(),
        password: "root".to_string(),
        namespace: "test".to_string(),
        name: "testdb".to_string(),
    };

    assert_eq!(config.connection_url(), "localhost:8000");
}

#[test]
fn test_server_socket_addr() {
    let config = ServerConfig {
        host: "127.0.0.1".to_string(),
        port: 3000,
    };

    let addr = config.socket_addr().unwrap();
    assert_eq!(addr.to_string(), "127.0.0.1:3000");
}
