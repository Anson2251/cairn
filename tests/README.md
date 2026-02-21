# Cairn API Integration Tests

This directory contains comprehensive integration tests for the Cairn backend API.

## Test Organization

The tests are organized into separate files by feature area:

### Test Files

1. **`auth_and_sketch_tests.rs`** - Authentication and sketch CRUD operations
   - User registration (with invite codes)
   - User login/logout
   - Get current user profile
   - Sketch creation, listing, retrieval, update, deletion

2. **`route_tests.rs`** - Route CRUD operations
   - Route creation within sketches
   - Route listing
   - Route retrieval
   - Route update (with version tracking)
   - Route deletion

3. **`invite_tests.rs`** - Invite code management
   - Invite code validation (public endpoint)
   - Create invite codes (admin only)
   - List invite codes (admin only)
   - Revoke unused invite codes (admin only)
   - Invite code format verification
   - Sequence increment verification

4. **`security_tests.rs`** - Security and hacking attempt tests
   - SQL injection attempts in various fields
   - XSS (Cross-Site Scripting) payload handling
   - Authentication bypass attempts
   - Privilege escalation attempts
   - Special characters and Unicode handling
   - Duplicate request idempotency

### Shared Test Utilities

**`common.rs`** - Shared test utilities and helper functions:
- `TestApp` struct - Test application wrapper
- `setup_test_db()` - Initialize test database
- `cleanup_test_db()` - Clean up test data
- `create_test_app()` - Create test application instance
- `create_test_user()` - Helper to register a test user
- `login_test_user()` - Helper to login a test user
- `create_admin_user()` - Helper to create an admin user
- `create_invite_code()` - Helper to create invite codes
- `create_test_sketch()` - Helper to create a sketch
- `create_test_route()` - Helper to create a route
- `send_request()` - Generic HTTP request helper

## Test Categories

### Regular Cases
- Normal API usage patterns
- Valid inputs and expected outputs
- Standard CRUD operations
- Proper authentication flows

### Edge Cases
- Empty inputs
- Boundary values (max length strings, etc.)
- Null/optional fields
- Pagination
- Invalid UUID formats
- Non-existent resources
- Concurrent modifications

### Security/Hacking Cases
- SQL injection attempts
- XSS payload handling
- Path traversal attempts
- Authentication bypass attempts
- Privilege escalation attempts
- Malformed JSON
- Oversized payloads
- Special characters and Unicode
- HTTP method override attempts

## Running the Tests

### Prerequisites

1. PostgreSQL must be running locally on port 5432
2. Redis must be running locally on port 6379
3. A test database will be created automatically (`cairn_test`)

### Run All Tests

```bash
cargo test
```

### Run Specific Test File

```bash
cargo test --test auth_and_sketch_tests
cargo test --test route_tests
cargo test --test invite_tests
cargo test --test security_tests
```

### Run Specific Test

```bash
cargo test test_register_success
cargo test test_sql_injection
```

### Run Tests with Output

```bash
cargo test -- --nocapture
```

### Run Tests with Backtrace on Failure

```bash
RUST_BACKTRACE=1 cargo test
```

## Test Configuration

Tests use the following configuration:
- **Database**: `postgresql://postgres@localhost:5432/cairn_test`
- **Redis**: `redis://localhost:6379`
- **Rate Limits**: Disabled (high limits for testing)
- **JWT Secrets**: Test-only secrets
- **Invite Salt**: Test-specific salt

## Test Isolation

Each test:
1. Creates a fresh test database
2. Runs migrations
3. Executes test logic
4. Cleans up all test data

This ensures tests are isolated and don't interfere with each other.

## Adding New Tests

To add new tests:

1. Add test functions to the appropriate test file
2. Use the helper functions from `common.rs`
3. Always call `cleanup_test_db()` at the end of each test
4. Follow the naming convention: `test_<feature>_<scenario>`

Example:
```rust
#[tokio::test]
async fn test_new_feature() {
    let app = create_test_app().await;
    // ... test logic ...
    cleanup_test_db(&app.db_pool).await;
}
```

## Notes

- Tests assume the Cairn server is NOT running (they use the router directly)
- Each test creates its own database instance for isolation
- Tests clean up after themselves to avoid database pollution
- The test database is dropped and recreated for each test
