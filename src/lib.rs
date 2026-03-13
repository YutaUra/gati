// Re-export modules needed by benchmarks.
// The main binary is in main.rs; this lib.rs exists only so that
// `benches/` and integration tests can access internal types.

pub mod git_status;
pub mod tree;
pub mod unicode;

#[cfg(test)]
pub mod test_helpers;
