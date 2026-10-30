# RustAPI implementation plan

Prepared **2026-10-29** for RustAPI `70b02c0`. This is proposed work, not an
implementation-complete checklist. The [feature gap analysis](feature-gap-analysis.md) contains
31 gap IDs, repository evidence and prioritized workflow requirements.
The [compatibility contract](import-compatibility.md) remains the description of
what currently works.

Work proceeds **one bounded step at a time**. The
[progress record](implementation-progress.md) identifies the completed checkpoint,
validation results and exact next slice. Finish and verify that slice before opening
another package; the milestone table describes dependencies, not parallel execution.

## Delivery order and scope

Keep Rust/egui, the native workspace format and the shared GUI/CLI execution engine.
Improve migration and the daily REST workflow before adding independent services.
No UI framework rewrite or mandatory account is needed for the first milestones.

| Milestone | Packages in dependency order | Result / exit gate |
| --- | --- | --- |
| M0: trustworthy migration | E00 → E01 | Current baseline recorded; supported imports preserve semantics; unsupported/lost data is explicitly reported before saving. |
| M1: everyday REST workflow | E02 → E03 → E04 → E05 → E06, after E01 | Saved examples, richer editing/inspection, common scripts, diagnostics, OAuth improvements and supported collection round trips work through GUI and CLI. |
| M2: local API lifecycle | E07 after E02/E05; E08 after E02; E09 after E05 | OpenAPI import/contracts, local generated docs, local mocks and reproducible run assets. |
| M3: selected extensions | E10 after E09; E11 after E01/E03/E05 | Bounded load testing and individually validated protocol clients. Pick protocols from actual project demand. |
| M4: optional platform | E12, only after a separate scope decision | Git UX, capture and other local extensions can ship separately; hosted services, organizational features, Flows and AI require their own designs. |

P0/P1/P2/P3 in the gap analysis indicate priority, not severity of every missing
feature. In particular, clear v3 rejection is an M0 requirement; implementing v3
is an M1 interoperability task. Dates and staffing are not committed: estimate each
package after its format/runtime spike and fixture inventory, and split large
packages into independently reviewable changes.

## Architecture and data contracts

1. **Version domain data deliberately.** Add descriptions, path parameters,
   multipart-part metadata and saved examples without breaking existing files.
   Additive defaults may stay compatible; incompatible changes require a schema
   bump, migration, backup and clear rejection by older readers. Never silently
   reinterpret an old serialized enum or rewrite the original imported file.
2. **Separate adapters from the native model.** Split `src/import.rs` into native,
   collection v2.1, collection v3, cURL and later OpenAPI adapters. Return a structured
   report containing source path, feature, severity and disposition: preserved,
   converted, unsupported or omitted. Do not place credential values in reports.
3. **Prepare requests once.** Introduce a shared prepared HTTP request abstraction
   for execution, effective-request inspection and code generation. Resolve scopes,
   URL/path/query encoding, auth precedence and multipart metadata consistently.
   Keep serializable configuration separate from acquired credentials and bytes.
4. **Add execution events.** Carry request/run/tab identity through logs, timing,
   assertions and progress. Use bounded event queues consumed by GUI and CLI.
   Preserve cancellation and independent tab attribution; do not expose raw secrets
   just to implement a console.
5. **Model reusable assets explicitly.** Saved examples contain a request snapshot
   and response status/headers/content. Do not serialize `ApiResponse` blindly:
   its raw bytes currently use `skip_serializing`. Store large example bodies as
   bounded managed assets with explicit content types and stable IDs. Datasets,
   specs and run profiles need their own versioned records; run history remains local.
6. **Keep protocol sessions distinct.** GraphQL-over-HTTP can reuse the HTTP pipeline.
   WebSocket, SSE, gRPC and MQTT need session/message/event models with bounded
   buffers; do not force them into one completed response object.

Proposed modules are suggestions, not files that already exist: `import/`,
`export/`, `execution/`, `models/example.rs`, `models/run_profile.rs`, `specs/`,
`mock/`, `protocols/`, and focused `ui/` panels. Extract functionality from the large
`src/app.rs` as each feature is added rather than performing an unrelated rewrite.

## E00 — Record the current baseline

**Scope:** delivery prerequisite; do not reopen already fixed historical findings.

- Inventory sanitized v2.1 collections/environment exports and minimal v3 fixtures
  covering representative REST workflows. Include both supported cases and loss cases.
- Record current toolchain, commit, GUI/CLI behavior and platform checks. Existing
  validation documents are historical evidence, not proof for a future feature branch.
- Establish fixture expectations for imported fields, emitted HTTP traffic, scripts,
  example bytes and failure reports. Keep account tokens and real response data out.

**Touchpoints:** `tests/fixtures/`, `tests/workflows.rs`, `tests/transport.rs`,
`tests/workspace.rs`, `tests/cli.rs`, `docs/validation.md`.

**Acceptance:** baseline results are tied to a commit and platform; unsupported
fixtures are clearly distinguished from expected passes. No claim of full parity
is derived from a fixture count.

## E01 — Preserve imported data and request semantics

E01.1 known-omission reporting and GUI/CLI review, and E01.2 plain-text row description
preservation, are implemented. Remaining field preservation and broader format/transport
correctness are pending; see the [progress record](implementation-progress.md).

**Priority:** P0. **Gaps:** G01–G03, G06. **Depends on:** E00.

- Preserve parameter/header/form descriptions, request/folder descriptions, structured
  path-variable definitions, all files in multipart arrays and per-part MIME types.
- Inventory imported request settings (`protocolProfileBehavior`) and unsupported
  metadata. Convert supported semantics explicitly; report every material omission.
  Until E02 lands, saved examples must be identified as unsupported rather than lost
  without a specific warning. Keep the imported source untouched.
- Recognize v2.1 JSON, native JSON and unsupported v3/YAML inputs. Give actionable
  format errors and the v2.1 export fallback; do not claim v3 support yet.
- Distinguish raw URL query text from structured query rows to avoid double-encoding
  or duplication; define `:id` path substitution separately from `{{variable}}`.
- Define duplicate/header override behavior, show effective headers and keep actual
  sends and snippets consistent. Preserve repeated fields where supported; report
  unsupported semantics instead of silently collapsing data.
- Surface an import summary in the GUI and machine-readable diagnostics in CLI.
  Material loss should require an explicit proceed choice; CI should fail by default
  unless a documented lossy-import option is selected.

**Touchpoints:** `src/import.rs`, `src/models/request.rs`, `src/models/collection.rs`,
`src/variables.rs`, `src/http/client.rs`, `src/http/code_gen.rs`, `src/app.rs`, `src/cli.rs`.

**Acceptance:** fixtures prove either preservation or an exact field-level diagnostic;
multiple files and descriptions survive save/reload; loopback captures prove encoded
paths, repeated parameters, header precedence and part content types; rejected imports
write nothing. Existing TLS defaults, atomic saves and identity tests remain passing.

## E02 — Saved examples, descriptions and workspace documents

**Priority:** P1. **Gaps:** G03, G16, G18. **Depends on:** E01.

- Add `RequestExample` with stable ID/name, request snapshot, response status,
  headers, content type and body/asset reference. Support several examples per request.
- Add Save response as example, Add/Edit/Duplicate/Delete example, example children
  in the tree and Try in a new request tab. Example editing must not issue a request.
- Import saved responses and their original request details. Preserve binary
  bytes where the supported source format allows them; diagnose unsupported encodings.
- Add request/folder Markdown descriptions, a local workspace README and document
  navigation. Start with safe Markdown rendering; hosted collaboration is excluded.
- Require an explicit save of response content; redact known credentials and preview
  what will be persisted/exported. Retain history's existing omission of response bodies.

**Touchpoints:** `src/models/request.rs`, `src/models/collection.rs`, proposed example
model, `src/storage/file_storage.rs`, `src/ui/response_viewer.rs`, `src/ui/tabs.rs`,
focused new UI panels, import/export adapters.

**Acceptance:** a sanitized equivalent of the screenshot's example workflow supports
two named responses (200 and 404), editing, restart, import/export and Try without
mutating the parent request. Deleting/reordering examples cannot corrupt open tabs.
Documentation renders offline; large bodies are bounded and missing assets are reported.

## E03 — Everyday editor and response inspection

**Priority:** P1. **Gaps:** G05–G08, G16. **Depends on:** E01; example UI uses E02.

- Reuse `KeyValue.description`; add a description column, bulk text mode and header
  presets. Switching modes must preserve order, duplicates, disabled rows and metadata.
- Add header-name suggestions, custom method entry, path-parameter editing and
  bidirectional query synchronization with documented encoding behavior.
- Highlight `{{variables}}`; show origin scope and resolved value on demand, masking
  secrets. Add completion and unresolved-variable diagnostics before sending.
- Add syntax highlighting, line numbers/folding, JSON tree, raw view and structural
  JSON filtering. Retain full-byte download and clearly identify preview truncation.
- Add workspace-wide search across names, URLs and descriptions; exclude secret/local
  state from indexes. Add history search, favorites and useful request breadcrumbs.
- Later viewer increments: XML formatting/filtering, image preview, inert HTML
  preview, hex view and response comparison. Treat full visualizer execution separately.

**Touchpoints:** `src/ui/request_editor.rs`, `src/ui/response_viewer.rs`, `src/variables.rs`,
`src/models/request.rs`, `src/app.rs`, proposed reusable editor/search components.

**Acceptance:** table↔bulk round trips retain data; URL↔query edits do not duplicate
parameters; variables show the correct scope; searching does not reveal vault contents;
large responses remain usable with honest truncation notices. Exercise keyboard-only
navigation and clipboard behavior in native GUI smoke tests.

## E04 — Collection interoperability, including v3

**Priority:** P1. **Gaps:** G01, G04. **Depends on:** E01/E02.

- First implement a documented **v2.1 collection and environment exporter**, with
  default credential redaction and explicit handling of RustAPI-only assertions/settings.
- Build a conformance table: supported unchanged, converted with notice, preserved
  opaque data, rejected. A round trip must never quietly discard unrecognized content.
- Spike the official **v3 multi-file YAML schema**, then implement a separate directory
  importer: request types, folder metadata, ordering, scripts and example resources.
  Unsupported protocols should be identified, not coerced into HTTP requests.
- Bound parser expansion/file counts; validate canonical resource paths, symlinks,
  case collisions and traversal. Import as managed copies, retaining source provenance.
- Add v3 export only after importer/fixtures stabilize. Until then document one-way
  support accurately and preserve the v2.1 migration route.

**Touchpoints:** `src/import.rs` split into adapters, proposed `src/export/`,
`src/storage/file_storage.rs`, `src/cli.rs`, import/export UI and compatibility docs.

**Acceptance:** supported v2.1 exports validate against the official schema and retain
semantic request/example/environment data on reimport; redaction is tested separately
from fidelity. v3 fixtures preserve order and resources; invalid paths cannot read or
overwrite outside the allowed import boundary. Validate selected fixtures in an
independent compatible client as a separate interoperability gate, recording its
version; RustAPI round trips alone cannot prove external format compatibility.

## E05 — Scripts, assertions and useful diagnostics

**Priority:** P1 for common workflows; P2 for async/packages. **Gaps:** G07, G09,
G13, G14. **Depends on:** E01 and E00 script fixtures.

- Add a bounded, redacted console with script phase, owning collection/folder/request,
  run/iteration ID and severity. Expose the effective request, auth/header origins,
  transport errors and existing timing fields. Add deeper timings only where measured.
- Add globals and a documented initial set of dynamic generators, with injectable
  clock/randomness for deterministic tests. Preserve precedence and scope lifetimes.
- Separate tracked environment defaults from persistent local overrides under `.local/`.
  Local overrides win within the environment scope, below iteration/local script values;
  keep secret values as vault references. Show both origins in the inspector and exclude
  overrides from exports unless explicitly selected. Cloud sharing remains E12 scope.
- Expand assertions from observed migration needs: common Chai comparisons/negation,
  typed equality and JSON Schema response validation with an explicit supported dialect.
- Add a script compatibility report and common `pm.request` read/mutation operations.
  Document script ordering, data writeback and the point at which a request is resolved.
- Stage `pm.execution` skip/branch control separately, with a bounded step count and
  cancellation. Add `pm.sendRequest`/async only behind a host-operation contract that
  preserves transport policy, timeout, cancellation and secret boundaries.
- External packages, `pm.vault` access and a full visualizer need separate capability
  designs; do not implement them by removing QuickJS's current limits.

**Touchpoints:** `src/scripts.rs`, `src/variables.rs`, `src/runner.rs`,
`src/models/workflow.rs`, proposed execution events, console/test-results UI.

**Acceptance:** sanitized imported scripts execute with expected ordering and scope;
unsupported APIs fail explicitly; schema mismatch appears in GUI and JSON/JUnit;
infinite loops, runaway branches and cancellation terminate deterministically.
Local environment overrides survive restart, resolve with the stated precedence and
stay out of tracked/exported defaults.
Console fixtures with known secrets prove redaction. No async/package parity claim
until those operations have their own passing fixtures.

## E06 — OAuth, authentication, snippets and cookie controls

**Priority:** P1 OAuth/common snippets; P2 remaining helpers. **Gaps:** G10–G12.
**Depends on:** E01 prepared-request/header semantics.

- Support configurable OAuth client authentication (body or Basic header), extra
  authorization/token/refresh parameters, separate refresh URL and a local token
  inspector. Keep PKCE state validation and session-only tokens as the default.
- Add JWT signing and AWS SigV4 as separate changes; then Digest if required by
  imported projects. Defer OAuth 1, NTLM, Hawk, EdgeGrid/ASAP until a real fixture or
  target API requires them. Do not add legacy OAuth grants solely for checkbox parity.
- Add Go `net/http`, C# `HttpClient`, Java OkHttp and raw HTTP snippets first; add
  Dart and other library variants next. Provide options/copy/save, with explicit
  diagnostics for settings a target runtime cannot reproduce.
- Add richer cookie editing and request cookie policy; make any persisted jar an
  explicit local setting excluded from Git. Browser capture belongs to E12.
- Consider a per-host certificate manager after the current request-level PEM controls.

**Touchpoints:** `src/models/auth.rs`, `src/models/workflow.rs`, `src/http/oauth.rs`,
`src/http/client.rs`, `src/http/code_gen.rs`, auth/cookie/codegen UI,
`scripts/codegen-smoke.py`.

**Acceptance:** fixture OAuth servers cover both client-auth modes and refresh;
signed requests match known vectors; samples compile/run in their declared runtimes
and emit equivalent loopback traffic including files and auth. Cookie isolation,
redaction and cancellation remain covered. Missing SDK/toolchain validation is reported.

## E07 — OpenAPI, contract checks and generated documentation

**Priority:** P1 import/local docs; P2 full specification workflow.
**Gaps:** G08, G14, G17, G18. **Depends on:** E02/E05.

- Import OpenAPI 3.0/3.1 JSON/YAML into requests, folders, variables, auth hints,
  descriptions and examples. Handle local references and report unsupported constructs.
- Add a local spec asset and validation panel; share schema evaluation with response
  assertions while distinguishing spec syntax/lint failures from response violations.
- Preserve operation identity so regeneration can preview changes without overwriting
  hand-edited tests/requests. Add contract drift reporting before automatic sync.
- Generate offline Markdown and static HTML docs from collections/examples, then
  add spec-derived docs. Publishing is an explicit export/deployment step, not automatic.
- Defer full visual spec editing, reusable organizational components, AsyncAPI/RAML/WSDL
  adapters and SDK/server generation until the OpenAPI path is dependable.

**Touchpoints:** proposed `src/specs/`, OpenAPI import adapter, shared validator,
document renderer/exporter, `src/cli.rs`, spec/docs UI.

**Acceptance:** OpenAPI fixtures with local `$ref`, security alternatives, path/query
parameters and multiple response examples produce usable requests. Invalid/cyclic
references fail clearly; remote references require explicit fetching policy. Regeneration
shows a diff and preserves user changes. Exported docs open offline and redact secrets.

## E08 — Local mocks from examples

**Priority:** P2. **Gap:** G19. **Depends on:** E02; OpenAPI-derived mocks use E07.

- Add a loopback-only mock service and CLI command using the same example model.
  Match method/path first, then explicit query/header/body rules with deterministic
  precedence; select named examples/statuses and configure delays/errors.
- Add start/stop/status, unmatched-request diagnostics and bounded request logs.
- Later add isolated state per session and bounded programmable handlers. Hosted
  URLs, access control, quotas and multi-user isolation belong to E12.

**Touchpoints:** proposed `src/mock/`, example model, `src/cli.rs`, mock-management UI.

**Acceptance:** an external HTTP client receives exact configured status/headers/body;
ambiguous matches are explained; ports shut down cleanly; two mock sessions cannot leak
state. The desktop and headless process use the same matching engine.

## E09 — Reusable runs, datasets, history and scheduling

**Priority:** P2. **Gaps:** G15, G20. **Depends on:** E05.

- Persist `RunProfile`: collection/folder/request selection, environment reference,
  dataset reference, iterations, delay and failure policy. Stable IDs survive renames.
- Add managed local CSV/JSON datasets with preview/validation, then reusable transforms
  if demanded. SQL/database-backed datasets are a separate scope, not assumed parity.
- Store bounded, redacted run summaries and comparisons; expose CLI report selection.
- Start scheduling through documented cron/systemd/launchd/CI invocations of the CLI.
  Add an in-app schedule only if background execution/restart semantics are specified.
- If alerts are added, make destinations/configuration explicit and deduplicate failures;
  distinguish local execution from an always-on hosted monitor.

**Touchpoints:** proposed run-profile/dataset models, `src/runner.rs`, `src/cli.rs`,
`src/storage/file_storage.rs`, runner UI, automation documentation.

**Acceptance:** a saved profile produces equivalent desktop/CLI outcomes after restart;
bad data/IDs fail before execution; histories enforce retention/redaction. Scheduled
jobs handle cancellation, overlap, timezones and missed runs predictably. Alerts are
tested with local sinks, not real recipients.

## E10 — Performance testing

**Priority:** P2. **Gap:** G21. **Depends on:** E09.

- Build a separate load executor that reuses request preparation but isolates cookies,
  variables and credentials per virtual user. Support explicit concurrency, duration,
  ramp/rate limits and an immediate stop control.
- Report throughput, error categories, p50/p95/p99 latency and client resource limits.
  State whether connection reuse and body transfer are included in measurements.
- Keep functional assertion results separate from the load summary.

**Touchpoints:** proposed load executor/report model, runner services, CLI and load UI.

**Acceptance:** deterministic local targets validate limits, cancellation, sample
counts and percentile calculations. No unbounded queues; user-state isolation is tested.
Do not market serial iteration support or timing assertions as completed load testing.

## E11 — Protocol expansion in independent slices

**Priority:** P2, demand-driven. **Gaps:** G22–G24. **Depends on:** E01/E03/E05.

| Slice | Implementation | Acceptance |
| --- | --- | --- |
| GraphQL first | Query/variables/operation-name model, imported GraphQL body mapping, schema fetch/import and completion using HTTP transport | Sanitized query/mutation fixtures round-trip; introspection-disabled servers work with supplied schemas; GraphQL errors remain distinguishable from HTTP errors. Subscriptions are a separate slice. |
| SSE | Incremental HTTP event parser, event view, reconnect policy and bounded buffers | Split UTF-8/chunk boundaries, event IDs, cancellation and long-lived streams behave predictably. |
| WebSocket | Connect/send/receive/close lifecycle, text/binary frames, auth/headers and transcripts | Echo fixtures cover disconnect/reconnect/cancel without cross-tab events or unbounded memory. |
| gRPC | Protobuf/reflection schema management, unary calls, then client/server/bidirectional streaming | Local service fixtures cover metadata, status/trailers, TLS and each claimed streaming type. |
| MQTT / Socket.IO | Separate session adapters and protocol-specific configuration; do not equate Socket.IO with plain WebSocket | Explicit broker/server fixtures cover subscriptions/events/reconnection and the supported protocol/version subset. |

**Touchpoints:** proposed `src/protocols/`, session/event models, protocol-specific UI,
import/export discrimination. Each slice updates the compatibility matrix separately.

## E12 — Optional local extensions and platform roadmap

**Gaps:** G12, G16, G18–G20, G25–G31. **Priority:** P2 for selected local extensions;
P3 for platform services. **Depends on:** relevant M1/M2 services and a scope decision.

| Workstream | First concrete step | Gate before claiming support |
| --- | --- | --- |
| Native Git UX and linked requests | File-change/conflict detection, local diff view and stable cross-collection references; external Git remains usable | External edits cannot overwrite dirty tabs; broken links are diagnosed; branch changes reload safely. |
| Traffic capture/browser cookies | HAR import first, then opt-in local capture service/browser adapter | Captured requests can be sanitized and saved; capture boundaries and certificate lifecycle are explicit. |
| External secret managers | Provider interface that resolves references at execution time | No secret material enters tracked files/logs; locked/offline/error states are visible. |
| Hosted docs/mocks/monitors | Design service API, identity, deployment, retention and tenant isolation | Authentication, access control and isolation tests pass; local clients remain usable without service access. |
| Team collaboration | Specify sync/conflict semantics, permissions, comments, activity and invitation flows | Concurrent/offline edits converge without data loss; authorization applies to every service operation. |
| Catalog/networks/inventory/reports | Define local service metadata and optional ingestion from specs/CI/UI tests | Ownership/search/access rules and provenance work on representative multi-service fixtures. |
| Governance/SSO/audit | Separate organization policy and identity design | Policies are enforced by service boundaries; audit records and identity provisioning are verifiable. |
| Visual Flows | Define graph serialization/execution on existing run services before building canvas UI | Branches, loops, data mapping, debugging and cancellation run reproducibly in GUI and CLI. |
| AI assistant / MCP / Context Graph | Separate AI assistance from protocol clients and from architectural indexing; define user-selected context and actions | Secrets/context remain bounded; proposed edits are reviewable; external actions are explicit; ordinary REST workflows work with AI disabled. |
| SDK/server generation | Evaluate spec-driven generators as an optional integration | Generated artifacts build against a pinned toolchain and are validated against the specification. |

These are discovery tasks with acceptance boundaries, not a promise to ship an
entire hosted platform in one release. Each needs its own implementation estimate
and operating model before scheduling.

## Validation and completion rules

- **Migration:** sanitized golden fixtures, semantic round trips, exact diagnostics,
  version migration and unchanged source files. Include unsupported cases deliberately.
- **Execution:** loopback HTTP/TLS/OAuth/protocol fixtures verify wire behavior,
  cancellation and errors. Keep real provider validation a separately recorded step.
- **UI:** exercise the screenshot-derived workflows in the packaged app, including
  examples, bulk edits, variable inspection, code copy, documents and restart recovery.
- **Automation:** same fixture results in GUI and CLI, correct exit codes, stable
  JSON/JUnit and no credential-bearing output. Compile/run added snippet targets.
- **Release:** run formatting, warnings-denied Clippy and relevant tests after final
  source changes; complete full CI/platform/package checks required by the repository.
  Record failures and untested platforms instead of carrying forward earlier results.
- **Documentation:** update README and compatibility contract as behavior lands;
  attach acceptance evidence and commit to each completed package. A working UI
  control, dependency addition or schema field alone does not complete its feature.

The next implementation change should be **E01's import report and fidelity fixtures**,
followed by its metadata/path/multipart fixes, each as a separate verified step.
Saved examples and editor improvements follow once that model is stable.
