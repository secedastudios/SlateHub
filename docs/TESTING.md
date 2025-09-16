# Testing Guide for SlateHub

## Overview

SlateHub uses a comprehensive testing strategy with isolated test environments for both unit and integration tests. The test setup ensures complete isolation between test runs and automatic cleanup of test data.

## Test Architecture

### Test Environment

The test environment consists of:
- **SurrealDB Test Instance**: Runs on port 8100 (vs 8000 for development)
- **MinIO Test Instance**: Runs on port 9100/9101 (vs 9000/9001 for development)
- **Isolated Test Data**: Separate directories for test data that are cleaned between runs
- **Test-specific Configuration**: Uses `.env.test` for test environment variables

### Test Types

1. **Unit Tests**: Test individual functions and modules in isolation
2. **Integration Tests**: Test complete workflows with real database and storage
3. **Model Tests**: Test database models with actual SurrealDB queries
4. **Route Tests**: Test HTTP endpoints with full request/response cycles

## Running Tests

### Quick Start

```bash
# Run all tests with automatic setup/teardown
make test

# Run only unit tests
make test-unit

# Run only integration tests
make test-integration

# Watch and auto-run tests on file changes
make test-watch

# Run a specific test file
make test-file FILE=user_tests

# Check test environment status
make test-status
```

### Manual Test Environment Management

```bash
# Start test environment (keeps it running)
make test-setup

# Stop test environment
make test-teardown

# Clean all test data and containers
make test-clean
```

## Writing Tests

### Test Organization

```
server/
├── tests/
│   ├── common/
│   │   └── mod.rs          # Shared test utilities and fixtures
│   ├── user_tests.rs       # User-related integration tests
│   ├── organization_tests.rs # Organization integration tests
│   └── project_tests.rs    # Project integration tests
└── src/
    └── [module]/
        └── mod.rs          # Unit tests in #[cfg(test)] modules
```

### Integration Test Example

```rust
use slatehub::models::user::User;
use slatehub::auth;

mod common;
use common::*;

#[tokio::test]
async fn test_create_user() {
    // Tests run in isolated environment with automatic setup/teardown
    with_test_db(|db| async move {
        // Create test data
        let email = "test@example.com";
        let username = "testuser";
        let password_hash = auth::hash_password("password").await?;
        
        // Use test fixtures
        let user_id = create_test_user(&db, email, username, &password_hash).await?;
        
        // Make assertions
        assert!(!user_id.is_empty());
        assert!(user_id.starts_with("users:"));
        
        // Query and verify
        let user = User::find_by_email(&db, email).await?;
        assert_eq!(user.email, email);
        
        Ok(())
    })
    .await
    .expect("Test failed");
}
```

### Unit Test Example

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_email() {
        assert!(is_valid_email("user@example.com"));
        assert!(!is_valid_email("invalid-email"));
        assert!(!is_valid_email("@example.com"));
    }

    #[tokio::test]
    async fn test_hash_password() {
        let password = "SecurePassword123!";
        let hash = hash_password(password).await.unwrap();
        
        assert!(!hash.is_empty());
        assert_ne!(hash, password);
        
        let is_valid = verify_password(password, &hash).await.unwrap();
        assert!(is_valid);
    }
}
```

## Test Helpers

### Common Test Fixtures

The `tests/common/mod.rs` module provides:

```rust
// Automatic test database setup/teardown
with_test_db(|db| async move {
    // Your test code here
    Ok(())
}).await;

// Test data creation helpers
create_test_user(&db, email, username, password_hash).await?;
create_test_org(&db, name, slug, owner_id).await?;
create_test_project(&db, title, creator_id, org_id).await?;

// Database cleanup
clean_database(&db).await?;

// MinIO setup/cleanup
setup_test_minio().await?;
cleanup_test_minio().await?;
```

### Environment Variables

Test environment uses these variables (automatically set by make commands):

```bash
DATABASE_URL=ws://localhost:8100/rpc
DATABASE_USER=root
DATABASE_PASS=root
DATABASE_NS=slatehub-test
DATABASE_DB=test
MINIO_ENDPOINT=http://localhost:9100
MINIO_ACCESS_KEY=slatehub-test
MINIO_SECRET_KEY=slatehub-test123
MINIO_BUCKET=slatehub-test-media
```

## Test Data Management

### Setup and Teardown

Each test file runs with:
1. **Setup**: Clean database, initialize schema, create MinIO bucket
2. **Test Execution**: Run test functions with isolated data
3. **Teardown**: Clean all test data, remove test containers

### Database Schema

Tests can use either:
- **Full Schema**: Loads from `db/schema.surql` if available
- **Minimal Schema**: Creates basic tables for testing

### Test Isolation

- Tests run with `--test-threads=1` to prevent conflicts
- Each test file gets a fresh database
- MinIO bucket is cleared between test files
- No state is shared between test runs

## Continuous Integration

### GitHub Actions

The project includes GitHub Actions workflows for:
- Running tests on every push/PR
- Code formatting checks with `cargo fmt`
- Linting with `cargo clippy`
- Coverage reports with `cargo-tarpaulin`

### Local CI Simulation

```bash
# Run the same checks as CI
cargo fmt -- --check
cargo clippy -- -D warnings
make test
```

## Coverage Reports

```bash
# Generate HTML coverage report
make test-coverage

# View report
open target/coverage/tarpaulin-report.html
```

## Debugging Tests

### Running Tests with Output

```bash
# Show print statements and logs
cargo test -- --nocapture

# Run specific test with output
cargo test test_create_user -- --nocapture

# Run with backtrace on failure
RUST_BACKTRACE=1 cargo test
```

### Test Environment Inspection

```bash
# Check if test containers are running
docker ps | grep test

# View test container logs
docker logs slatehub-surrealdb-test
docker logs slatehub-minio-test

# Connect to test SurrealDB
surreal sql --conn ws://localhost:8100 --user root --pass root
```

## Best Practices

### Do's ✅

- Write both unit and integration tests
- Use test fixtures for common setup
- Clean up resources in tests
- Test error conditions and edge cases
- Use descriptive test names
- Keep tests focused and independent
- Mock external services when appropriate

### Don'ts ❌

- Don't share state between tests
- Don't use production credentials in tests
- Don't skip cleanup in test failures
- Don't test implementation details
- Don't write overly complex test setups
- Don't ignore flaky tests

## Troubleshooting

### Common Issues

**Test environment won't start**
```bash
# Check if ports are already in use
lsof -i :8100
lsof -i :9100

# Force cleanup and restart
make test-clean
make test-setup
```

**Tests fail with connection errors**
```bash
# Ensure Docker is running
docker info

# Check test service health
curl http://localhost:8100/health
curl http://localhost:9100/minio/health/live
```

**Database schema issues**
```bash
# Verify schema file exists
ls -la db/schema.surql

# Check for syntax errors
surreal validate db/schema.surql
```

**MinIO bucket errors**
```bash
# Check MinIO console
open http://localhost:9101
# Username: slatehub-test
# Password: slatehub-test123
```

## Advanced Testing

### Performance Testing

```rust
#[tokio::test]
async fn test_bulk_insert_performance() {
    with_test_db(|db| async move {
        let start = std::time::Instant::now();
        
        // Insert 1000 records
        for i in 0..1000 {
            create_test_user(
                &db,
                &format!("user{}@example.com", i),
                &format!("user{}", i),
                "password_hash"
            ).await?;
        }
        
        let duration = start.elapsed();
        println!("Inserted 1000 users in {:?}", duration);
        
        // Assert performance threshold
        assert!(duration.as_secs() < 10, "Bulk insert too slow");
        
        Ok(())
    })
    .await
    .expect("Test failed");
}
```

### Load Testing

```bash
# Use cargo-criterion for benchmarks
cargo install cargo-criterion
cargo criterion

# Use drill for HTTP load testing
cargo install drill
drill --benchmark load_test.yml
```

### Security Testing

```rust
#[tokio::test]
async fn test_sql_injection_prevention() {
    with_test_db(|db| async move {
        // Attempt SQL injection
        let malicious_input = "'; DROP TABLE users; --";
        
        let result = db.query("SELECT * FROM users WHERE email = $email")
            .bind(("email", malicious_input))
            .await;
        
        // Should safely handle the input
        assert!(result.is_ok());
        
        // Verify users table still exists
        let users = db.query("SELECT * FROM users").await?;
        assert!(users.is_ok());
        
        Ok(())
    })
    .await
    .expect("Test failed");
}
```

## Contributing

When adding new features:

1. Write tests FIRST (TDD approach)
2. Ensure all existing tests pass
3. Add integration tests for new endpoints
4. Add unit tests for new functions
5. Update this documentation if needed
6. Run `make test` before committing

## Resources

- [Rust Testing Book](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Tokio Testing](https://tokio.rs/tokio/topics/testing)
- [SurrealDB Testing Guide](https://surrealdb.com/docs/integration/testing)
- [MinIO Testing Best Practices](https://min.io/docs/minio/testing)