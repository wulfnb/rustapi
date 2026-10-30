# RustAPI implementation progress

Updated: **2026-10-30**. Source baseline for the final requested step: `eaf6684`.

## Working agreement

- Implement one small, reviewable slice at a time.
- Run the checks appropriate to that slice before starting the next one.
- Record changed files, results, unresolved issues and the next action here so work
  can resume without depending on chat context.
- Keep completed work distinct from the proposed [implementation plan](implementation-plan.md).
- Commit and push each verified step to the same branch (`main`) before starting
  the next step, as requested on 2026-10-29.

## Step 1 — Independent product wording

Status: **complete**, verified locally and pushed to `origin/main` as `966ecd0`.

- Renamed the gap analysis, implementation plan, import compatibility and historical
  audit documents; updated their links and converted comparison copy to product requirements.
- Updated README, website capabilities, import labels, errors and script comments.
- Renamed the collection fixture and test identifiers; retained the v2.1 format marker
  with a neutral test schema URI. The parser and supported script APIs are unchanged.
- Scope is maintained repository content and filenames, excluding Git history and
  generated/ignored build artifacts. No deployment or publication is part of this step.

Validation:

- Content/filename scan: no removed naming remains in 112 maintained files.
- Markdown links, website anchors and all 31 gap IDs: passed.
- Fixture comparison: all request, auth, variable and script data retained; only the
  test schema URI changed, keeping the `v2.1.0` marker.
- `cargo fmt --all --check`: passed.
- `cargo test --locked --offline --test workflows`: **14 passed, 0 failed** on macOS.
  The initial sandboxed attempt passed 10 tests; four could not bind localhost sockets.
  The complete rerun with loopback access passed.
- `git diff --check`: passed.

Validation covers the workflow suite and static content checks, not full release or
packaged GUI validation. No new feature package is complete.

## Step 2 — E00 baseline and import fixture inventory

Status: **complete**, verified locally and pushed to `origin/main` as `688edcd`.

- Added sanitized fixtures for six HTTP body modes, environment values/disabled flags,
  known metadata omissions, and a synthetic unsupported YAML boundary.
- Added four integration tests for supported conversion, managed-copy CLI imports,
  source preservation, unsupported schema/body rejection and no imported data on failure.
- Documented the fixture classifications and precise limitations in
  [import baseline](import-baseline.md). Production importer behavior is unchanged.

Validation on macOS ARM64, Rust 1.98.0, against `966ecd0` plus this fixture-only change:

- `cargo test --locked --offline --all-targets`: **121 passed, 0 failed**, including
  native UI harness, CLI, workflow, transport, storage and fixture suites. Loopback
  tests ran with the required local socket access.
- `cargo clippy --locked --offline --all-targets -- -D warnings`: passed.
- `cargo fmt --all --check` and `git diff --check`: passed.
- Full platform packaging and coverage measurements remain separate release gates.

## Step 3 — E01.1 structured import diagnostics

Status: **complete**, verified locally and pushed to `origin/main` as `7f84ef8`.

- Added versioned, bounded JSON reports with structural source locations, stable
  feature codes, warning severity and omission/partial/unsupported dispositions.
  Reports do not copy imported names, values, file paths or response bodies.
- Report known missing descriptions, examples, path variables, behavior settings,
  multipart content types/extra files, variable metadata and unsupported auth.
- Desktop imports wait for explicit acceptance; cancellation and workspace changes
  discard pending imports without saving project data.
- CLI imports/runs reject known omissions with exit 2. `--allow-lossy-import` opts in;
  `--import-report` emits JSON on stdout for imports. This is not a dry-run option.
- Added six import-policy tests and two GUI interaction tests, including exact field
  locations, redaction, bounded reports, cancellation, acceptance and run environments.

Validation on macOS ARM64, Rust 1.98.0, against `688edcd` plus this implementation:

- `cargo test --locked --offline --all-targets`: **129 passed, 0 failed**.
- `cargo clippy --locked --offline --all-targets -- -D warnings`: passed.
- `cargo fmt --all --check` and `git diff --check`: passed.
- Naming scan over 119 maintained files and documentation-link checks: passed.
- Native UI harness tests cover the review actions; fresh packaged desktop interaction,
  cross-platform release checks and current coverage percentages are not claimed.

The report covers the documented known fields; an empty report is not a guarantee of
complete import fidelity. This checkpoint does not implement preservation of omitted
fields or the remainder of E01.

## Step 4 — E01.2 preserve plain-text row descriptions

Status: **complete**, verified locally and pushed as `d73cf4f`.

- Retain plain-text descriptions on headers, query rows, URL-encoded/multipart fields
  and environment variables, including disabled rows, Unicode and line breaks.
- Remove omission diagnostics only for descriptions that are now retained. Collection
  and folder variable descriptions still require review because their scope models use
  plain value maps; structured row descriptions also remain unsupported and reported.
- Add regression checks through real CLI imports, workspace save/reload and native
  collection reimport, proving descriptions alone do not need lossy-import acceptance.
- Keep remaining omission reports, report limits, redaction and unsupported metadata
  checks intact. Description editing in the request UI remains E03 work.

Validation on macOS ARM64, Rust 1.98.0, against `7f84ef8` plus this implementation:

- `cargo test --locked --offline --all-targets`: **131 passed, 0 failed**.
- `cargo clippy --locked --offline --all-targets -- -D warnings`: passed.
- `cargo fmt --all --check` and `git diff --check`: passed.
- Naming scan over 119 maintained files and documentation-link checks: passed.
- No native model/schema change or new platform/package validation was required by
  this import-metadata change; cross-platform release checks remain separate gates.

## Step 5 — E01.3 preserve request and folder descriptions

Status: **complete**, verified locally; delivery is recorded in Git history.

- Added defaulted description fields to request/folder models without changing the
  native schema version. Older files load with empty descriptions.
- Imported plain-text descriptions now survive storage, native reimport and request
  duplication. Structured descriptions remain explicitly reported until the next slice.
- Full local suite: **132 passed**, warnings-denied Clippy and formatting passed.

## Active scope and next step

The user requested completion of import correctness, daily workflow and compatibility
on 2026-10-30. [Delivery backlog](delivery-backlog.md) tracks every requested capability.
Continue with structured descriptions and scope-variable metadata, then path variables,
files, settings and URL/header semantics. Verify, commit and push each slice before
starting the next. Later protocol/hosted-platform work is outside these three groups.

## Step 6 — Structured descriptions and scope-variable metadata

Status: **complete**, verified locally; delivery is recorded in Git history.

- Preserve description content, format and extension metadata across collection,
  folder, request, row and environment storage. Encoding uses edited content with
  retained metadata rather than stale imported text.
- Preserve scope-variable rows, disabled flags, type hints and extensions alongside
  legacy value maps. Disabled rows cannot resolve or mask enabled ancestor values;
  script removals do not resurrect stale metadata values.
- Invalid description/type representations still require review. Diagnostic reports
  remain bounded and do not expose imported metadata values.
- Full local suite: **134 passed**; formatting and warnings-denied Clippy passed.
- Fixed a cancellation-test timing race by synchronizing cancellation after receipt
  of a request. The original full run failed when cancellation preceded connection;
  the corrected full rerun passed.

Next: path-variable substitution and encoding, then multipart files/MIME types.
The rest of the three-group scope remains active in the delivery backlog.

## Step 7 — Path-variable execution and editing

Status: **complete**, verified locally; delivery is recorded in Git history.

- Retain path-variable rows and metadata, expose them in Params, resolve scoped values
  and encode substitutions as individual URL path segments. Existing encoded path text,
  query parameters and fragments retain their roles.
- GUI/CLI execution and snippets share resolution. Missing, disabled, duplicate and
  dot-segment values fail before network I/O rather than altering routing unexpectedly.
- Storage/native reimport and real loopback traffic are covered.
- Full local suite: **136 passed**, warnings-denied Clippy and formatting passed.

Next: multiple multipart files and per-part MIME types, including generated samples.

## Step 8 — Multipart files and MIME types

Status: **complete**, verified locally; delivery is recorded in Git history.

- Expand imported upload arrays into ordered, independently editable repeated file
  rows. Preserve descriptions, disabled flags and per-part content types.
- Support multi-file selection and part MIME editing. Resolve variables and workspace
  paths before sending; validate MIME types and keep disabled uploads unread.
- The multipart encoder owns the body Content-Type/boundary. Existing cURL, Python,
  JavaScript and Rust generators preserve repeated parts and text/file MIME types.
- Full local suite: **138 passed**; warnings-denied Clippy and formatting passed.
- Python tooling: **15 passed**. All **24 generated HTTP requests passed**, with the
  multipart workload strengthened to check two file bodies and text/file MIME types.

Next: imported request settings and URL/header semantics.

## Step 9 — Imported request settings

Status: **complete**, verified locally; delivery is recorded in Git history.

- Apply collection → folder → item → request settings for redirects, cookie-jar use,
  TLS verification and timeout milliseconds. Explicit request overrides win.
- Preserve original profile metadata, including unsupported options, while reporting
  options the runtime cannot honor. Imported certificate-verification disablement
  requires explicit review. Malformed supported options fail parsing.
- Canonical request settings remain editable after import and are not overwritten by
  stale profile metadata on native reload. Cookie-jar policy has a desktop control.
- Full local suite: **140 passed**, warnings-denied Clippy and formatting passed.
  Local HTTP fixtures verify redirect and cookie behavior, not just serialization.

Next: shared URL/query and header repetition/override semantics.

## Step 10 — Shared URL and header semantics

Status: **complete**, verified locally; delivery is recorded in Git history.

- Structured query rows replace matching raw URL keys, including disabled rows,
  while unrelated encoded pairs and fragments remain intact. Structured URLs
  without a raw string are reconstructed with validated components.
- Sending and snippets share header validation, auth precedence and body MIME
  defaults. Repeated ordinary headers remain ordered; duplicate singleton headers
  fail early. Python/JavaScript snippets explicitly reject repeated fields that
  their map APIs cannot preserve. cURL/Rust retain them.
- Default export/history redaction includes path variables, typed-secret scope
  metadata and sensitive extension/settings fields.
- Full local suite: **144 passed**; formatting and warnings-denied Clippy passed.
  All **24 generated HTTP requests passed**.

The import-correctness group is complete within the documented supported subset.
Next: saved request/response examples, followed by the remaining daily workflow
features and compatibility group in the active delivery backlog.

## Step 11 — Saved request/response examples

Status: **complete**, verified locally; delivery is recorded in Git history.

- Import saved v2.1 responses and original request snapshots with ordered headers,
  status, extension metadata and bounded exact response bytes (including binary).
- Capture, rename, edit, duplicate and delete examples from the request editor.
  Save the parent to persist edits. Tree/example Try actions open an independent
  request tab and response without replacing the original request.
- Default capture/export redacts recognized sensitive fields; history drops examples.
  Duplication regenerates example IDs. Nested and oversized examples are rejected.
- Full local suite: **147 passed**; formatting and warnings-denied Clippy passed.
- The final suite exposed a macOS OAuth callback socket inheriting nonblocking mode;
  accepted sockets now explicitly use blocking reads with the existing timeout.

Next: workspace documents and description editing, then bulk rows/presets,
variable inspection and richer response views.

## Step 12 — Workspace documents and descriptions

Status: **complete**, verified locally; delivery is recorded in Git history.

- Offline Documents window reads/edits UTF-8 Markdown source with atomic saves,
  new-name copies, collision checks and workspace-specific local draft recovery.
- Document storage rejects traversal, symlink components and files over 1 MiB.
  Drafts are bounded separately; another workspace never loads this draft.
- Request, collection, folder and key/value-row descriptions are editable while
  imported format metadata remains intact. New row controls enable rows by default.
- Full local suite: **152 passed**; formatting and warnings-denied Clippy passed.
  A final targeted description UI rerun passed after a test-only lint cleanup.

Next: lossless bulk row editing and persistent header presets.

## Step 13 — Lossless bulk rows and header presets

Status: **complete**, verified locally; delivery is recorded in Git history.

- JSON bulk editors preserve ordered duplicate rows and every native metadata field;
  invalid or oversized input leaves the table untouched. Apply and Reload are explicit.
  Request/table IDs isolate staged JSON across contexts.
- Named header presets persist under workspace .local with append/replace/delete
  controls, schema/size limits, collision checks and shared header validation.
  Recognized credential fields require references rather than literal secrets.
- Full local suite: **155 passed**; formatting and warnings-denied Clippy passed.

Next: contextual variable/effective-header inspection and richer response views.

## Step 14 — Contextual inspection and richer responses

Status: **complete**, verified locally; delivery is recorded in Git history.

- Inspector shows winning variable origins and resolution errors, masking sensitive
  names, type-hinted secrets, vault values and aliases by default. Header values are
  all masked until explicit Reveal; preparation matches the shared send path.
- Response syntax, JSON folding/Pointer filter, raw hex, inert source preview,
  bounded PNG/JPEG decoding and header/body/total timing are available.
- Image input/dimension/pixel/allocation and tree/display budgets are documented;
  raw downloads retain full response bytes within existing transport limits.
- Full local suite: **159 passed**; formatting and warnings-denied Clippy passed.
  New tests cover origin precedence, secret aliases, error text, Pointer controls,
  pixel decoding and preview rejection bounds.

The requested daily-workflow group is complete within documented limits.
Next: portable v2.1/environment export, v3 directory support and the remaining
compatibility entries in the active backlog.

## Step 15 — Portable collection and environment export

Status: **complete**, verified locally; delivery is recorded in Git history.

- GUI/CLI export portable v2.1 collections and environments with default redaction;
  CLI including credentials is an explicit flag. Native JSON export remains.
- Preserve ordering, structured descriptions, disabled/scope metadata, path/query
  rows, repeated headers, multipart files/MIME, inherited settings, auth, scripts
  and saved examples. Binary bytes and native-only checks/extraction/settings use
  a versioned x-rustapi extension with an export note.
- Native-extension TLS disablement participates in import review. Export cannot
  overwrite its CLI input. The neutral version identifier and external-consumer
  extension limits are explicit in the compatibility contract.
- Full local suite: **163 passed**; formatting and warnings-denied Clippy passed.
  Tests include exact portable re-export equality, binary examples, native options,
  environment CLI redaction and the desktop shared-adapter export path.

Next: bounded v3 YAML collection-directory import/export.

## Step 16 — Bounded v3 collection import/export

Status: **complete for the documented HTTP subset**, verified locally; delivery is
recorded in Git history.

- GUI/CLI import/export and CLI runs accept v3 JSON/YAML/directory collections.
  Definition/request/example layout, numeric ordering, .yml aliases, inline scripts,
  supported auth, variables, bodies and settings map through shared services.
- Native sidecars restore extra metadata only after exact regenerated-YAML matching;
  edits trigger omission review instead of overriding new source values. Export
  stages a new directory and refuses an existing destination.
- Resource, parser and filesystem limits reject symlinks, ambiguous/colliding names,
  unsupported files/protocols/providers/packages/includes and expansion constructs.
- Full local suite: **169 passed**; formatting and warnings-denied Clippy passed.
  Headless v3 execution exercised real path/query encoding, scripts and HTTP traffic.
- Independently checked **11 emitted YAML documents** against published JSON
  definitions, excluding directory-only order indexes. This found and corrected
  folder auth inheritance representation. It is not a whole-ecosystem parity claim.
- Maintained-file naming/content scan passed.

Next: JSON Schema assertions, broader scripts/debugging, OAuth options and additional
code-generation targets.

## Step 17 — Offline JSON Schema assertions

Status: **complete for the documented bounded subset**, verified locally; delivery
is recorded in Git history.

- Tests UI edits whole-response or JSON Pointer-targeted schema assertions; shared
  execution reports failed checks consistently in desktop and headless runs.
- Maintained validator with external retrieval features disabled, local acyclic
  references, format validation, linear regex and schema/instance complexity limits.
- Error messages expose locations without response/expected values. Literal const
  data containing a $ref key is handled as data, not a schema reference.
- Schema checks persist through native/portable extensions.
- Full local suite: **172 passed**; formatting and warnings-denied Clippy passed.
  Tests cover valid/invalid schemas, formats, references, remote/file rejection,
  composition, UI selection and real shared/CLI HTTP assertion failures.

Next: broader synchronous scripting and bounded workflow control.

## Step 18 — Expanded synchronous scripts and workflow control

Status: **complete for the documented synchronous subset**, verified locally;
delivery is recorded in Git history.

- Common assertion chains, comparisons, types, object/array assertions, schema
  calls and response-header helpers perform real checks with bounded execution.
- Pre-request method/URL/query/header/raw-body mutation runs through the same
  resolution/HTTP path; post-response state is read-only and saved requests stay
  unchanged. Validate output before applying mutations, including prototype-like
  variable names as JSON data.
- Skip, stable-ID/unique-name next-request selection, null stop and step limits
  work in collection runs. JSON/JUnit/desktop views mark skipped requests.
  Skipped/error responses cannot become invalid saved HTTP examples.
- Full local suite: **176 passed**; final desktop suite **35 passed**, final example
  tests **2 passed**, formatting and warnings-denied Clippy passed.
- Actual HTTP tests cover method/query/header/body mutations; workflow tests cover
  branching, stops, ambiguity and terminating a 1000-step loop. Existing 14 workflow
  regressions remain passing. Async/package/auxiliary-request APIs remain explicit
  unsupported boundaries.

Next: bounded, redacted script/network debugging console.

## Step 19 — Bounded redacted debugging console

Status: **complete**, verified locally; delivery is recorded in Git history.

- Shared session log records network metadata, script log/warn/error and generic
  execution failures. Desktop filter/copy/clear and explicit CLI --debug-report
  expose the same redacted events; normal reports remain separate.
- Bound queue/events/script arguments, handle circular values and preserve logs
  before ordinary errors without applying failed mutations. Paths/query/body/header
  values and raw transport exceptions are not stored in network traces.
- Context redaction covers values, sensitive JSON keys, nested JSON strings and
  computed keys. Limits and encoded/custom-data caveats are documented.
- Full local suite: **181 passed**; formatting and warnings-denied Clippy passed.
  Tests prove ordering, redaction of request/response/variable/object values, logs
  before failures, VM/queue caps, CLI opt-in and desktop copy/clear.

Next: OAuth provider/client-auth options, then additional code-generation targets.

## Step 20 — OAuth client authentication and provider options

Status: **complete for the documented grant/callback subset**, verified locally;
delivery is recorded in Git history.

- Body/Basic/public client choices, separate refresh URL/scope and scoped provider
  parameter lists persist and round-trip through portable credentials. Default body
  mode preserves old native files. Basic encoding and single-method submission follow
  RFC 6749; managed state/PKCE/grant/client fields cannot be overridden.
- Token exchanges reject redirects and invalid Bearer tokens. Browser callbacks
  validate path/state/duplicates and bounded fragmented headers; authorization results
  bind to the resolved configuration and verifier.
- Desktop resolves OAuth variables before browser launch, with editor controls and
  stale-result rejection. Request/provider/endpoint redaction includes assertion
  credentials, preserves unrelated raw query/template text and omits persistent
  history error details that could otherwise expose endpoint credentials.
- Full local suite: **190 passed**; final targeted redaction/model/workspace checks:
  **30 passed**. Formatting and warnings-denied Clippy passed. Real HTTP tests
  cover Basic encoding, public PKCE, refresh routing/scope, provider fields, redirect
  rejection and callback fragmentation/validation.

Next and final requested capability: raw HTTP, Go, C#, Java and Dart code targets.

## Step 21 — Additional executable code targets

Status: **complete for the documented target subset**, verified locally;
delivery is recorded in Git history. All items in the active delivery backlog
are implemented; broader platform roadmap features remain separate scope.

- Added raw HTTP, Go, C#, Java and Dart to the existing request code panel, with
  wrapped target controls. Shared resolution/auth/header semantics feed escaped
  language literals and byte-preserving upload/multipart builders.
- SDK targets implement redirect policies, one network deadline and response bounds.
  CA/client-certificate/proxy/TLS policies are emitted where supported; unsupported
  options fail explicitly. Raw HTTP uses exact framing and rejects external uploads.
  See the [capability matrix](code-generation.md) for all compatibility limits.
- Extended the executable fixture with repeated headers, control/Unicode separators
  and policy profiles. The added driver compiles real SDK code against isolated HTTP,
  TLS and proxy fixtures, including trusted-chain wrong-host rejection, mutual TLS,
  delayed bodies and same/cross-origin credential handling. Certificates are temporary.
- Final local Rust suite: **196 passed**; documentation tests, formatting and
  warnings-denied Clippy passed. Python tooling: **15 tests**, **100% statement/branch
  coverage**. Actionlint, maintained-content naming and Markdown link checks passed.
  The final desktop build passed isolated native event-loop startup/shutdown smoke.
- Generated requests: existing four targets **24 passed**, additional targets/policies
  **70 passed**. Local SDKs: Go 1.27.3, .NET 8.0.427, Java 17 and Dart 3.13.7.
  A fresh short-lived certificate fixture resolved Apple platform certificate-policy
  failures from the older long-lived test certificate. C# validates URI hostname and
  server-auth usage independently alongside chain trust.
- Added Linux CI execution of the extra SDK driver, pinning patched Dart 3.13.7.
  Hosted CI, other-platform SDK execution and packaged GUI/release validation were
  not observed in this step and are not implied by the local results.

## Step 22 — Editor and variable improvements

Status: **complete for the documented REST client workflow**, verified locally.

- **Custom HTTP methods:** Added `HttpMethod::Custom(String)` with bidirectional string
  serialization/deserialization, custom method entry in the URL bar, wire execution,
  and snippet generation across cURL, JavaScript, Python, Rust, Go, C#, Java and Dart.
- **URL/query synchronization:** Added bidirectional real-time synchronization between
  the URL bar query string and the structured query parameters table, preserving disabled
  parameters and unescaped variable templates.
- **XML/HTML editing tools:** Added `BodyType::Xml` and `BodyType::Html` with automatic
  `application/xml` and `text/html` Content-Type headers, format-preserving code generation,
  and offline XML formatting (`format_xml`) with tag/attribute indentation.
- **Global scope & dynamic values:** Expanded `Variables` with `global` scope (precedence:
  `global < collection < environment < local overrides < iteration < local < vault`),
  support for Postman-compatible dynamic generators (`$guid`, `$timestamp`, `$isoTimestamp`,
  `$randomInt`, `$randomEmail`, etc.), and script runtime integration with `pm.globals`.
- **Persistent local overrides:** Created `.local/globals.json` and
  `.local/environment-overrides.json` storage with schema validation, 1 MiB bounds, and
  atomic writes, allowing environment values to be overridden locally without modifying
  or checking in tracked environment files.
- **Autocomplete & hover inspection:** Added standard HTTP header suggestions, variable
  hover tooltips with scope identification and secret masking, and variable completion palette.

Validation:
- Local Rust test suite: **204 passed, 0 failed** (including new `tests/editor_improvements.rs` and `tests/variable_improvements.rs`).
- Warnings-denied Clippy: passed.
- `cargo fmt --all --check` and `git diff --check`: passed.
