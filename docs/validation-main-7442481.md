# Validation — 2026-10-29

## Merged CI/CD

The initial CI/CD changes are merged in `8359bb4a640be57aed6323a1acdcda59679ffda0`.
[GitHub Actions run 40756562326](https://github.com/wulfnb/rustapi/actions/runs/40756562326)
completed successfully, including:

- Linux x64, macOS ARM64, macOS Intel, and Windows x64 native checks.
- Docker tests, actual desktop/browser checks, and persistent-volume validation.
- Native release packaging and launches from extracted release archives.
- GHCR publication, checksums, and build-provenance attestations.

The resulting [preview release](https://github.com/wulfnb/rustapi/releases/tag/preview-8359bb4a640b)
contains six native archives and `SHA256SUMS.txt`. Its published Docker digest is
`sha256:6e9f460e9bcc4fed60312f6bd9deb7fe94b5ac03113ccedb00f5893f7feb14af`.
These published artifacts precede the GPLv3 and coverage follow-up branch.

## GPLv3 and coverage follow-up

The following checks apply to `feature/gplv3-and-full-coverage`. Local Rust coverage
was collected on Debian Linux ARM64 in the reproducible coverage image, using the
pinned Rust 1.98.0 and cargo-llvm-cov 0.9.1. The hosted Ubuntu x64 quality job runs the
same test and coverage commands when a pull request is opened.

| Check | Result |
| --- | --- |
| Rust unit, CLI, model, code-generation, HTTP, storage, and GUI tests | 89 passed, 0 failed |
| Full production Rust line coverage | 2,645 / 2,645 — 100% |
| Full production Rust function coverage | 246 / 246 — 100% |
| Python release and Docker tooling | 11 tests passed; 177 / 177 statements and 50 / 50 branches — 100% |
| Shell desktop launcher | 9 / 9 executable lines; ready, retry, failure, and argument-preservation cases passed |
| Real Linux native dialogs | Import/export, both cancellations, source preservation, and import persistence passed |
| Native desktop lifecycle | Event-loop startup, normal close, state.json and history.json persistence passed |
| Rust formatting and all-target Clippy with warnings denied | Passed |
| Documentation tests | Passed; currently no executable documentation examples |
| Actionlint, ShellCheck, and Python syntax | Passed |
| Reproducible coverage image | Built successfully for Linux ARM64 |

Coverage includes all production Rust modules, including `main.rs`, the application,
and UI. No source exclusions or coverage-disable annotations were added. LLVM region
coverage is 99.75%; stable Rust branch coverage is not available in this run. A 100%
line/function result does not establish every possible input or platform-specific
backend behavior. Native platform checks remain a separate CI matrix.

Machine-readable Rust JSON and LCOV, Python JSON, shell JSON, and native screenshots
and logs are retained locally under the ignored `coverage/` directory. CI uploads
these as `coverage-linux` and enforces the Rust, Python, and shell thresholds.

The follow-up branch has not yet completed a hosted PR run. The successful main run
above verifies the preceding release pipeline; it does not substitute for CI on this
new branch. macOS packages remain ad-hoc signed rather than notarized, Windows ZIPs
remain portable unsigned executables, and published Docker images target amd64.
