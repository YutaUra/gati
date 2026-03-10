---
description: "Enforces Red-Green-Refactor TDD cycle. Use when writing or modifying any implementation code."
paths:
  - "src/**/*.rs"
  - "tests/**/*.rs"
---

# Test-Driven Development (Red-Green-Refactor)

## Purpose

All code changes MUST follow the Red-Green-Refactor cycle. Never write implementation code without a failing test first.

## Rules

### Required

1. **Red first**: Write a failing test BEFORE writing any implementation code. Run the test and confirm it fails.
2. **Green minimally**: Write the minimum implementation code to make the failing test pass. Do not add extra logic.
3. **Refactor safely**: After Green, refactor only while all tests remain passing. Do not add new behavior during refactoring.
4. **One test at a time**: Do not write multiple tests before making the first one pass. Complete each Red-Green-Refactor cycle before starting the next.
5. **Verify Red**: Always run the test after writing it to confirm it actually fails. A test that passes immediately is not driving the design — investigate why.
6. **Verify Green**: Always run the test after writing implementation to confirm it passes. Do not assume it works.

### Workflow

```
[Write test] → [Run test → FAIL ✗] → [Write minimum code] → [Run test → PASS ✓] → [Refactor] → [Run test → PASS ✓] → repeat
```

## Examples

### Bad — Writing implementation first

```rust
// ❌ Writing implementation without a test
pub fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(512).any(|&b| b == 0)
}

// Then adding a test after
#[test]
fn test_is_binary() {
    assert!(is_binary(&[0x00, 0x01]));
}
```

### Good — Red-Green-Refactor cycle

```rust
// Step 1 (Red): Write the test first
#[test]
fn detects_binary_when_null_byte_present() {
    let data = vec![0x48, 0x65, 0x00, 0x6C];
    assert!(is_binary(&data));
}
// → Run: cargo test → FAIL (is_binary does not exist) ✗

// Step 2 (Green): Minimum implementation
pub fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(512).any(|&b| b == 0)
}
// → Run: cargo test → PASS ✓

// Step 3 (Refactor): Improve if needed, tests still pass
// → Run: cargo test → PASS ✓

// Step 4: Next cycle — write the next failing test
#[test]
fn detects_text_when_no_null_bytes() {
    let data = b"Hello, world!";
    assert!(!is_binary(data));
}
// → Run: cargo test → ?
```

**Why**: Writing tests first ensures the test actually validates something meaningful. If you write the implementation first, tests become after-the-fact confirmations that may not catch real bugs. The Red step proves the test has value — it can fail.
