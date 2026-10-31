# Remaining-feature validation

Date: 2026-10-31. Checkout: canonical RustAPI `main`, based on `650cd4d`.
This records local validation before commit/push; consult Git history and Actions
for subsequent delivery. No hosted CI run, packaged release, remote deployment or
replacement of the installed application is claimed by this validation record.

The supported workflows and boundaries are documented in
[advanced tools](advanced-tools.md) and [authentication](authentication.md).

## Local evidence

| Check | Result |
| --- | --- |
| `cargo test --locked --all-targets` through cargo-llvm-cov | 244 tests passed, 33 suites, zero failures |
| Rust coverage including native smoke and independent protocol processes | Lines 81.21% (12536/15437); functions 77.64% (1184/1525); regions 75.87% |
| Existing Rust coverage gates | Passed 75% lines and 75% functions; thresholds unchanged, production code not excluded |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `cargo fmt --all --check`, `git diff --check` | Passed |
| Documentation test compilation | Passed; no doctest examples defined |
| Independent Aedes, Socket.IO and grpc-js tests | Six cases passed: MQTT pub/sub, MQTT rejection of an untrusted TLS certificate, authenticated Socket.IO echo, gRPC unary/server stream/event cap |
| Asymmetric JWT signatures checked by OpenSSL | RSA, RSA-PSS, P-256, P-384 and Ed25519 passed; fixed HMAC vector independently computed with Python stdlib |
| Native macOS desktop | Isolated startup and clean shutdown smoke passed with the final dependency tree |
| Browser connector and website JavaScript | 15 tests passed |
| Python tooling | 16 tests passed; 100% statement and branch coverage (288 statements, 82 branches) |
| Workflow/shell validation | Actionlint and ShellCheck passed |
| Source secret scan including untracked files | No leaks; three public synthetic JWT/HMAC vectors have line-specific explanations/allow markers |
| RustSec dependency audit | Zero vulnerability findings; no advisory ignores added |

Coverage used cargo-llvm-cov 0.9.1, Rust 1.98.0, an isolated temporary build directory,
and debug information disabled to avoid duplicating the large normal build cache.
Additional native and protocol processes emitted their own coverage profiles.
Region coverage is reported separately and is not a branch-coverage claim.

The dependency audit initially identified vulnerable JWT cryptography and an old
MQTT certificate verifier. JWT now uses jsonwebtoken's AWS-LC backend; MQTT uses
native TLS. Vulnerable RSA/crypto-bigint and rustls-webpki 0.102 dependencies were
removed, and the yanked Windows clipboard package was updated. The audit still
reports transitive maintenance warnings and existing unsoundness advisories for
event-listener 5.4.1, memmap2 0.9.9 and rand 0.8.5/0.9.2. These are not suppressed;
zero vulnerability findings does not mean the dependency tree has no advisory warnings.

## Desktop and service workflows exercised

Desktop tests use isolated workspace directories, real egui controls and injected
file selections at the native-dialog boundary. They cover specification loading,
lint/import, disk-change overwrite protection, draft recovery, documentation output,
mock save/load/start/stop, saved profiles/datasets/history, load configuration,
protocol profile save/load and live SSE, flow build/edit/save/reload/run, and team
token/configuration/server/catalog/push/pull/policy controls. A vault regression
verifies that saving local secrets never persists external/session credential caches.

Loopback integration tests cover Digest auth-int with exact binary bytes, signed
redirect boundaries and shared deadlines, OAuth encrypted restart/refresh/forget,
cookie attributes/encryption/bridge origin and token checks, response contracts,
mock matching/delays/faults, scheduler locking/resume behavior, concurrent isolated
virtual-user sessions, hard execution caps, role/expiry/revision/audit enforcement,
Vault and SigV4 AWS secret-provider requests, unsent AI drafts, MCP discovery and
actual stdio concurrency/cancellation, and a failure/recovery visual flow.

## Independent collection compatibility

Postman CLI 1.83.1 accepted the exported HTTP v3 fixture with **zero errors and zero
warnings**. The directory produced by its v2.1 migration also passed its linter and
imported into RustAPI without omission approval.

On the final sandboxed macOS migration invocation, the external CLI wrote its files
and then exited with a system-keychain `SecItemCopyMatching` error/crash. The fixture
was separately linted and imported successfully; successful completion of that
external migration process is not claimed. No Postman account or cloud service was
used. These fixtures do not prove arbitrary v3 resources or script parity.

## Validation limits

Hosted Linux, Windows and Intel macOS CI/package results have not been observed for
these changes at this validation checkpoint. CI now includes the browser connector tests and the
independent Linux protocol driver with pinned npm dependencies and checksum-verified
grpcurl 1.9.4. No new release assets or website deployment were created.

The Chromium connector's browser API operations are regression-tested alongside a
real paired HTTP bridge; an installed browser-extension capture was not performed.
Ollama/Vault/AWS behavior uses HTTP fixtures rather than production accounts/models.
No SSO service, remote team reverse proxy, public documentation deployment or OS
scheduler service was provisioned. See the guide for OpenAPI/protocol subsets and
the external grpcurl/model requirements.
