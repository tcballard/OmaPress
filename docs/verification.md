# Verification record

Local verification on 5 September 2026, Linux x86_64 container, Rust 1.98.0 and Qt 6.8.3.

- 21 Rust engine regressions pass.
- 6 integration tests pass using the real CLI and real Git with controlled GitHub/HTTP transport. Includes verified publish, correction, rollback, failed pushes, unknown verification, remote movement and dirty worktree preservation.
- Native C++/QML application builds and opens; its sample-publication screenshot was inspected.
- Formatting and Clippy pass with warnings denied.
- Combined stripped executables: 5,123,504 bytes at the first package check.
- Synthetic 1,000-text-article build: 0.207 s. Single-article preview p95: 12.16 ms including process startup. These are container observations, not XPS hardware acceptance.

The loopback HTTP integration test must run in CI because this local environment restricts socket creation. Real Omarchy keyboard/accessibility, live theme changes, X paste and RSS reader integration remain Tom’s acceptance tasks. Domain ownership/spelling and real article import remain pending user-supplied material.
