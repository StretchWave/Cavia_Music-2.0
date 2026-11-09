//! Ensures the library links into an integration test target on Windows and
//! that the test-only Common-Controls manifest is applied without conflicting
//! with the application manifest.

#[test]
fn library_links_into_integration_test_target() {
    assert!(std::mem::size_of::<cavia_lib::domain::Track>() > 0);
}
