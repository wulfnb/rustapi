# RustAPI: local-first Postman alternative audit

> Historical baseline audit of main before implementation. See [current capabilities](../README.md)
> and [compatibility boundaries](postman-compatibility.md) for the implementation on this branch.
> The [2026-10-29 comparison](postman-comparison.md) and
> [implementation plan](postman-implementation-plan.md) describe the newer checkout's remaining gaps.

Reviewed on 2026-10-28 against freshly fetched `origin/main`, commit
`8359bb4a640be57aed6323a1acdcda59679ffda0`.
Branch: `review/main-gap-audit-2026-10-28`.
Worktree: `/private/tmp/rustapi-postman-review`.

The first-release target was confirmed by the user: a local-first REST client with
collections, environments, authentication, Postman import, tests, and CLI runs.
RustAPI has a useful native request editor and HTTP transport foundation, but is
not yet ready to replace Postman for that workflow. Data integrity is the first
release gate, followed by migration and executable test workflows.

## Current capability and missing work

| Capability | Current main | First-release requirement |
| --- | --- | --- |
| REST requests | GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS; params, headers, JSON/text and text form fields | Preserve these; configurable timeout, redirects and per-request cancellation; accurate transport errors |
| Collections and folders | JSON collections, duplicate/rename/delete; nested folders exist only in models | Stable identifiers, folder CRUD/navigation, move/reorder, reliable disk persistence |
| Request editing | Multiple tabs; Save available for collection-backed tabs | Save new/history requests into a collection; dirty indicators; save/discard/cancel on close; recover drafts after restart |
| Environments | Enabled environment variables with basic string replacement | Collection/environment/local and iteration scopes with documented precedence; unresolved-variable diagnostics; deterministic resolution |
| Authentication | Basic, bearer, header/query API key | Collection/folder inheritance and request overrides; OAuth 2 authorization-code with PKCE and client credentials, token expiry/refresh |
| Transport configuration | Fixed 30-second timeout; new blocking client per send; certificate validation disabled | TLS verification by default, explicit development override, custom CA/client certificates, proxy configuration and reusable clients |
| Cookies | No configured shared cookie store or cookie UI | Workspace cookie jar and inspect/edit/clear controls for login workflows |
| Bodies and files | JSON, text, URL-encoded, multipart text | Multipart file fields and actual binary bytes/file uploads with content type |
| Response inspection | Status, headers, text/JSON, timing and size | Search/copy/download; binary-safe saving; bounded previews and large-body handling; total body-transfer timing |
| Postman migration | Import deserializes RustAPI's own Collection JSON only | Postman collection v2.1 and environment JSON import, nested folders, auth, variables and bodies; explicit warnings for unsupported scripts/features |
| Other migration | No cURL or OpenAPI importer | cURL import for initial release; OpenAPI import after the core migration path |
| Assertions and extraction | No assertion, script, extraction or test-result model | Status/header/JSON/body/time assertions; response-to-variable extraction; pass/fail results on single sends and runs |
| Scripts | No pre-request or post-response runtime | A documented compatibility boundary; time-limited JavaScript subset for common imported `pm.*` scripts, with unsupported APIs reported rather than discarded |
| Collection runs | No runner | Ordered collection/folder runs, selected environment, variable chaining, CSV/JSON iteration data, cancellation and result reports |
| CLI and automation | Only `--help`, `--version`, GUI and desktop smoke mode | Headless `run`, environment/data selection, JSON/JUnit reports and failure exit codes; same engine as the GUI |
| Local-first workspaces | One application data directory; `RUSTAPI_DATA_DIR` override | Open/create a project folder, versioned schema and migrations, portable Git-friendly collections, local secret references separate from tracked files |
| Secrets | Credentials serialized in request/environment/history JSON | Secret references backed by OS credential storage or an encrypted local store; redacted history/export/log defaults and explicit opt-in to include secrets |
| Distribution | Native/Docker CI and release workflows exist | Complete passing platform/package checks; accurate license and release documentation |

Scope decisions: no required account or cloud service for the first release.
Cloud sync, live collaboration, RBAC, hosted mocks, monitors, documentation publishing,
GraphQL tooling, WebSocket, gRPC and plugin systems are later milestones. JSON GraphQL
requests can already be sent as ordinary HTTP bodies; specialized protocol tooling
is a separate feature. Full Postman script/API compatibility is not an initial claim.

## P0: fix before treating the app as a dependable daily client

P0 here means a release blocker involving data loss, response attribution or the
transport trust default. P1 means necessary for the agreed usable REST release.

1. **Deleting earlier items can make a tab overwrite unrelated data.** Tabs retain
   `collection_idx` and `request_idx`. Deletion removes vector entries but never
   adjusts later tabs. Reproduction: create A/B/C, open B, delete A, edit/save B;
   C is overwritten with B. The same happens when deleting an earlier collection.
   Confirmed with executable probes. Use collection/request IDs for identity and
   resolve them on every operation; cover deletion, move and reorder.
   Evidence: [app.rs](../src/app.rs#L131), [request deletion](../src/app.rs#L171),
   [collection deletion](../src/app.rs#L195), [tab identity](../src/ui/tabs.rs#L68).

2. **Async responses and history can belong to the wrong request.** The worker sends
   only `ApiResponse`. Completion applies it to the tab active at that moment and
   records that tab's current request, not the sent snapshot. Send A then switch to
   or edit B while A is pending. Source tracing confirms there is no originating
   tab/request/run ID in the message. Capture the originating tab ID, immutable
   request snapshot and execution ID; route results by identity. Track loading and
   cancellation per request. Enter can also initiate another request while the Send
   button is disabled; the loading guard must exist in the execution path.
   Evidence: [message](../src/app.rs#L12), [send](../src/app.rs#L108),
   [completion](../src/app.rs#L230), [Enter handling](../src/ui/request_editor.rs#L201).

3. **Collection files collide and new collections do not retain their saved path.**
   `save_collection` chooses a name-derived filename and overwrites it. Two collections
   with the same name, or names that sanitize to the same string, share a file.
   `create_collection` ignores the returned path; deleting a just-created collection
   does not delete its file, so it returns after restart. Both defects were reproduced.
   Use stable unique filenames and retain canonical paths after each successful save.
   Evidence: [save](../src/storage/file_storage.rs#L95),
   [create/delete](../src/app.rs#L188), [delete file](../src/storage/file_storage.rs#L197).

4. **TLS certificate verification is disabled for every request.**
   `danger_accept_invalid_certs(true)` is unconditional. Verify certificates by default;
   expose an explicit development-only override, with a visible indicator. Verified
   by source inspection, not a dedicated TLS handshake probe.
   Evidence: [HTTP client construction](../src/http/client.rs#L10).

5. **Saving/importing can silently lose or change data.** Save errors are discarded,
   and `save_current_request` marks a tab saved even when disk writing fails. A probe
   reproduced the cleared dirty flag after a deliberately failed write. Writes use
   direct `fs::write`, without atomic replacement/recovery. Import retains the source
   file path, so subsequent edits overwrite that external file instead of creating a
   managed local copy; reproduced with an external temporary file. Make copy versus
   open-in-place explicit, surface errors, mark saved only after success, and use
   atomic writes plus schema-versioned recovery/migration.
   Evidence: [save](../src/app.rs#L131), [import UI](../src/app.rs#L281),
   [storage writes](../src/storage/file_storage.rs#L106),
   [import function](../src/storage/file_storage.rs#L230).

## P1: finish the existing behavior as well as adding features

- **Saving new requests is missing and edits are not tracked.** The Save control
  requires `has_save_target`; new/history tabs have none. No production edit handler
  sets `is_modified = true`, and close/exit do not save or confirm drafts. Add Save As,
  change tracking, close protection and recoverable session drafts.
  Evidence: [Save visibility](../src/ui/request_editor.rs#L52),
  [tab closing](../src/app.rs#L470), [state saved on exit](../src/app.rs#L216).
- **Environment rename duplicates persisted environments.** Filenames include name
  plus ID prefix. Save under a new name leaves the old file; delete removes only the
  first matching file. Reproduced two copies after rename and one remaining after
  deletion. Use an ID-based path and migrate duplicates safely.
  Evidence: [save](../src/storage/file_storage.rs#L150),
  [delete](../src/storage/file_storage.rs#L204).
- **Renaming a background request can rename the active tab's request instead.** The
  rename dialog receives the active request name regardless of its rename target.
  Resolve and update the matching tab by request ID, preserving its unsaved changes.
  Source inspection: [caller](../src/app.rs#L396), [dialog at the audited baseline](https://github.com/wulfnb/rustapi/blob/8359bb4a640be57aed6323a1acdcda59679ffda0/src/ui/modals.rs#L94).
- **A truncated response is shown as successful empty content.** `response.text()`
  errors become an empty body and `error: None`. A loopback server advertising 100
  bytes but sending only `abc` reproduced status 200 with empty body and no error.
  Propagate body-read failures. Timing currently stops before the body is read, and
  response bodies are loaded without a size bound. Preserve raw bytes and separate
  transfer duration from time to headers.
  Evidence: [response handling](../src/http/client.rs#L162).
- **Generated code does not reliably reproduce the configured request.** Probes
  confirmed query API keys are absent in Python/JavaScript/Rust, multipart fields
  absent in JavaScript/Rust, Python JSON uses undefined `true`/`null` names, cURL
  appends parameters after a URL fragment, and Rust OPTIONS generates a nonexistent
  `client.options(...)` helper. String escaping is also incomplete across URL/header/
  credential/body interpolation. Build all outputs from one resolved request model;
  test syntax and the emitted request against loopback servers for every language.
  Evidence: [code generators](../src/http/code_gen.rs#L59),
  [existing tests only inspect selected strings](../tests/code_gen.rs#L25).
- **Binary/file support is incomplete.** `Binary` is absent from `BodyType::all()`,
  file selection is a TODO, the request body is a String, and multipart fields are
  always text. Existing HTTP tests exercise string bytes, not arbitrary file bytes.
  Evidence: [types](../src/models/request.rs#L84),
  [file UI](../src/ui/request_editor.rs#L155), [multipart](../src/http/client.rs#L141).
- **Invalid headers are silently omitted.** Parsing failures in configured and auth
  headers do not become request validation errors. Report them before sending.
  Evidence: [header handling](../src/http/client.rs#L68).

## Implementation sequence and acceptance gates

1. **Reliable request lifecycle and storage.** Introduce stable IDs and an execution
   service; fix all P0 findings, atomic persistence, Save As and draft protection.
   Gate: delete/reorder while tabs are open cannot alter unrelated requests; switching
   or closing tabs during a slow request never misattributes a response; disk failure
   preserves unsaved state and reports an actionable error; same-named collections
   persist independently; TLS verification is on by default.
2. **Postman migration and project model.** Add a versioned workspace format, nested
   folders, collection variables/auth inheritance, Postman collection v2.1/environment
   imports, and cURL import. Keep project data and local secrets separate.
   Gate: representative export fixtures retain methods, URLs, params, bodies, folders,
   auth and variables; unsupported scripts are preserved with visible warnings; input
   files are never modified by a copy import; imports validate before writing anything.
3. **Daily REST completeness.** Add real multipart/binary files, OAuth 2, cookies,
   configurable HTTP settings and response search/download; correct code generation.
   Gate: local fixture servers validate uploads, login/cookie reuse, OAuth token refresh,
   redirect/proxy settings and every emitted code sample without public API dependencies.
4. **Tests and chained runs.** Add serializable assertions/extractors and run results,
   then a collection/folder runner and bounded script support. Share variable resolution
   and request execution with interactive sends. Gate: a login -> extract token ->
   authenticated request flow works with ordered execution, reports assertion failures,
   supports data iterations and stops/cancels as configured. Publish the supported
   `pm.*` subset; unsupported imported APIs fail clearly instead of silently passing.
5. **Headless CLI and release.** Expose the same runner through a headless CLI with
   environment/data selection, JSON/JUnit results, documented exit codes and Ctrl-C.
   Gate: the same collection produces equivalent GUI/CLI results; assertion or transport
   failures return a nonzero exit status; an offline project opens without an account;
   packaged Windows/macOS/Linux checks complete successfully.

Keep the existing Rust/egui foundation. Put a versioned domain/workspace model,
resolved-request representation, execution service, storage, import adapters and
runner behind library APIs; GUI and CLI should be adapters over these services.
A UI rewrite is not required to address these gaps. A JavaScript engine, if added,
needs explicit time/memory limits and filesystem/network capability boundaries;
API script compatibility must be tested separately from runtime availability.

## Validation performed for this audit

- `cargo fmt --all --check`: passed.
- `cargo clippy --locked --offline --all-targets -- -D warnings`: passed.
- `cargo test --locked --offline --all-targets`: all **64 existing tests passed** on
  local macOS ARM64. The initial sandboxed attempt could not bind loopback sockets
  or access the macOS network configuration API; the unrestricted rerun passed.
- Four temporary executable audit tests confirmed **13 defect observations** across
  app state, file storage, generated code and a deliberately truncated loopback HTTP
  response. These probes asserted the existing bad behavior, so a passing probe is
  confirmation of a defect, not proof of correctness. All profiles were temporary.
  The probe sources were removed after execution; application sources are unchanged.
- No new full GUI interaction run, Docker rebuild, release packaging or local coverage
  measurement was performed. The existing validation document reports 39.71% line
  coverage for an older local run; this is not a fresh measurement for this audit.
- GitHub main CD run: https://github.com/wulfnb/rustapi/actions/runs/40756562326.
  At inspection, native macOS ARM64, Windows and Ubuntu plus quality/coverage had
  passed; Intel macOS and Docker were still running. No published releases were
  returned by `gh release list`. These are point-in-time observations, not completed
  release validation.

## Documentation and repository gaps

- `README.md` claims MIT but main contains no LICENSE file; Cargo metadata still has
  placeholder author text and lacks license/repository metadata. Separate local work
  on `feature/gplv3-and-full-coverage` adds GPLv3-or-later documentation, but it is not
  on main. Preserve and reconcile that work rather than independently choosing a license.
- `docs/ci-cd.md` still says the old `33dda88` commit is the latest remote main, and
  `docs/validation.md` describes already-merged changes as uncommitted. Refresh these
  records after the active main run completes, keeping old local evidence distinct.
- There is no product roadmap or migration compatibility contract on main. This audit
  supplies the initial release scope and acceptance gates; actual implementation is
  outstanding. Do not label the current client a complete Postman replacement.

## Postman reference points

These sources establish the workflow being replaced; the release scope above is
our selected subset rather than a claim of feature parity:

- [Import formats and migration](https://learning.postman.com/docs/getting-started/importing-and-exporting/importing-data/)
- [Collection and environment JSON exports](https://learning.postman.com/docs/getting-started/importing-and-exporting/exporting-data/)
- [Variable scopes and precedence](https://learning.postman.com/docs/tests-and-scripts/write-scripts/postman-sandbox-reference/pm-variables/)
- [Pre-request/post-response scripts and assertions](https://learning.postman.com/docs/tests-and-scripts/write-scripts/test-scripts/)
- [Ordered collection runs, chaining, iteration data and automation](https://learning.postman.com/docs/tests-and-scripts/running-collections/intro-to-collection-runs/)

No product fixes, commits, pushes, PRs or publishing were performed by this audit.
This audit did not edit source files or switch branches in the original checkout.
Other work on its existing feature branch was excluded from this main-based review.
