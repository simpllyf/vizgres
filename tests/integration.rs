//! Integration test runner
//!
//! To run these tests:
//! 1. Start the test database: docker-compose -f docker-compose.test.yml up -d
//! 2. Run tests: cargo test --test integration
//!
//! Environment variables (with defaults):
//! - TEST_DB_HOST: localhost
//! - TEST_DB_PORT: 5433
//! - TEST_DB_NAME: test_db
//! - TEST_DB_USER: test_user
//! - TEST_DB_PASSWORD: test_password

#[path = "integration/postgres_tests.rs"]
mod postgres_tests;
