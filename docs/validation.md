# Local validation — 2026-10-28

These results apply to the uncommitted CI/CD and regression-test changes based on
`33dda88e283430d24016db1762503dc69bb96d14`.

| Check | Result |
| --- | --- |
| macOS ARM64 unit/integration tests | 64 passed, 0 failed |
| Debian Linux ARM64 tests in Docker | Same 64 tests passed, 0 failed |
| Rust formatting and all-target Clippy with warnings denied | Passed |
| Actionlint 1.7.12, ShellCheck 0.11.0, Python syntax | Passed |
| Compose configuration | Passed |
| macOS optimized build | Passed |
| macOS archive | Ad-hoc signature verified, ZIP extracted, packaged app opened the request editor and completed its event loop |
| macOS DMG | Created and verified with hdiutil |
| Final Docker image | Built from final source, ran as UID 1000, real app window present |
| Docker file chooser | FileChooser portal available on the running RustAPI D-Bus session |
| Docker browser viewer | HTTP viewer and WebSocket-to-VNC handshake passed |
| Docker persistence | Profile-volume marker survived container restart |

The 64 tests comprise 7 URL-builder tests, 7 tab/history UI tests, 3 CLI tests,
6 code-generation tests, 19 loopback HTTP tests, 18 model/utility tests, and
4 storage tests. Storage tests and native smoke checks use isolated temporary profiles.

The full macOS LLVM test run measured **39.71% line coverage**. HTTP execution measured
94.27% and file storage 91.05%. The full-project percentage includes unexercised GUI
paths; separate native startup checks are not included in that coverage measurement.
Machine-readable LCOV and a summary are retained locally under `coverage/` (ignored
build output). CI will generate its own Linux coverage report.

Local macOS downloads are under `dist/`: the ARM64 ZIP, DMG, and `SHA256SUMS.txt`.
These generated files are ignored by Git. The validated local container is available
as `rustapi:local`.

Windows x64, Intel macOS, GitHub's Linux x64 runner, release publishing, GHCR pushes,
and attestations have **not** been executed remotely. They are configured in the
workflows and need a push to the selected repository. No commits, pushes, releases,
repository settings, or server deployments were performed during local validation.
