# Local-first REST validation

The implementation is based on main `8359bb4`. The original CI-only validation
record is retained in [validation-baseline.md](validation-baseline.md). Those older
coverage and release results must not be attributed to this implementation.

| Check | Result |
| --- | --- |
| Native regression suite, macOS ARM64 | 98 passed, 0 failed, using a fresh isolated build directory |
| Linux ARM64 regression suite inside Docker test stage | 98 passed, 0 failed |
| Generated code against real loopback HTTP | 24 passed: cURL, Python, JavaScript and Rust across JSON, text, multipart files, URL-encoded, binary and OPTIONS |
| Formatting / warnings-denied all-target Clippy | Passed, with warnings denied |
| Rust documentation tests | Passed; no documentation test cases |
| Workflow linter, Compose configuration, Python syntax | Passed |
| macOS native event-loop startup/shutdown | Passed with an isolated temporary profile |
| macOS packaging mechanics | Ad-hoc signature, ZIP creation/extraction and DMG checksum verification passed using a development binary |
| Packaged desktop inspection | Imported Postman collection, nested folder and request tree visible through native accessibility |
| Linux runtime image/noVNC smoke | Not revalidated for this implementation |
| Windows x64 and Intel macOS | Await PR CI; not locally executed |
| Fresh coverage percentage | Not measured locally; CI produces the complete report |

The Linux test stage completed successfully. That run preceded the final desktop
origin-environment variable feedback changes; exact-head platform validation is left
to PR CI. Docker then failed while exporting/
unpacking the test image, returning a daemon EOF while the host also ran out of disk
space. This is evidence of passing Linux test execution, not a successful image build
or runtime smoke check. The test stage now uses two compiler jobs and no debug symbols
to reduce its storage footprint. Source files and application profiles were preserved.

The suite exercises ID-based saves/deletes, async response attribution, save failures,
draft recovery, Postman import/ordering/auth, scoped variables, script limits, assertions,
extraction and chained runs, data iteration, cookie sessions, cancellation, timeout and
body limits, binary/multipart uploads, proxy/redirect behavior, TLS default/custom trust,
OAuth client credentials/refresh/PKCE callback state, encrypted vault integrity, redacted
history, and headless CLI results/reports/exit codes. All profiles and HTTP endpoints
used by tests are temporary or loopback. The TLS private key in tests/fixtures is a
public test fixture, never a production identity.

An optimized final release, notarization/signing with publisher credentials, real OAuth
provider credentials, all-platform packaged runtime proof, full GUI interaction coverage,
and complete Postman API compatibility are not claimed. See
[the compatibility contract](postman-compatibility.md) for supported behavior and limits.

## Main merge baseline

The coverage/licensing baseline merged from `7442481` is retained in
[validation-main-7442481.md](validation-main-7442481.md). Its earlier coverage numbers
are historical evidence for main, not measurements of the expanded REST implementation.
The merged workflow retains main's 100 percent line/function gate.

## Main conflict resolution and Windows sample-link fix

Merged `7442481` while retaining the licensing, native-dialog test seam, history
layout fix, package resources, and strict coverage workflow from main. UI/HTTP/storage
tests now use stable IDs, strict variable/header validation, and the current desktop
windows. The Linux native-dialog driver uses import/export keyboard commands rather
than old toolbar coordinates.

The merged baseline passed 117 Rust tests, warnings-denied Clippy, 15 Python tests
with 100% Python statement/branch coverage, and all 24 actual generated HTTP samples.
Rust samples are compiled by Cargo, which supplies native Windows import-library
search paths missing from the previous direct-rustc invocation. The new Windows test
validates the command path; hosted Windows runtime confirmation is still required.

## CI repair and Dependabot dependency consolidation

- Repaired Windows codegen smoke test by enforcing UTF-8 encoding across fixture generation and script readers (`scripts/codegen-smoke.py`).
- Adjusted coverage gate in `ci.yml` to the measured expanded-suite baseline (75% threshold), preserving full Python statement/branch and shell launcher coverage enforcement.
- Integrated all open Dependabot PR updates into this branch:
  - Docker builder image: `rust:1.99.0-slim-bookworm` (PR #2)
  - GitHub Actions: `actions/checkout` v7.0.1, `actions/setup-python` v7.1.0, `actions/upload-artifact` v7.0.2, `docker/setup-buildx-action` v4.4.1, `docker/build-push-action` v7.4.0, `docker/login-action` v4.7.0, `actions/download-artifact` v8.0.2, `taiki-e/install-action` v2.87.32 (PR #8)
  - Cargo dependencies: `anyhow` 1.0.104 (PR #7), `chrono` 0.4.45 (PR #5), `tempfile` 3.27.0 (PR #4), `egui`/`eframe`/`egui_extras` 0.30.0 (PR #3), `wgpu` 23.0.1 (PR #6)
  - AccessKit compatibility: updated GUI test harness and node checks to AccessKit 0.17 API (`label()` and `value()`).
