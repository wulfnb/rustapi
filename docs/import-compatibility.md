# Import compatibility and first-release boundaries

RustAPI targets a local-first REST workflow. The supported import format is
collection v2.1 JSON plus environment JSON, alongside native RustAPI JSON and
cURL commands. There is no account requirement. Cloud/team features, hosted monitors,
mocks, OpenAPI generation and specialized gRPC/WebSocket/GraphQL tooling remain later
milestones. This release does not claim that arbitrary imported collections run unchanged.

For supported format boundaries, known import-fidelity gaps and
future work, see the [feature gap analysis](feature-gap-analysis.md) and
[implementation plan](implementation-plan.md). RustAPI imports
v2.1 JSON and a bounded v3 HTTP subset in JSON/YAML and directory form.

## Import and project data

Imports retain nested folder/request ordering, enabled parameters/headers/form fields,
raw bodies, file references, collection/environment variables, inherited supported auth,
and collection/folder/request pre/post scripts. Bodies support raw JSON/text, URL-encoded,
multipart and file modes. Unrecognized methods/body modes stop the import before files
are written. Collection v1/v2.0 documents must be exported as v2.1 first.

String and structured descriptions retain their text, format and extension metadata.
Scope-variable rows retain descriptions, type hints, disabled flags and extensions;
legacy maps remain authoritative for active values. Disabled rows are preserved but
excluded from resolution. Script changes do not resurrect removed values when rows
are reconstructed for export. The desktop edits request, collection, folder and row descriptions while retaining their format metadata.

Scripts are preserved as source and their compatibility warning is visible after import.
OAuth requires local authorization and upload paths may need adjustment. Unsupported
auth types produce a warning and require explicit configuration before use. Dynamic
variables such as `{{$randomInt}}` and global variables are not implemented; unresolved
references are errors. Unknown native collection schema versions are rejected.

Portable project files carry stable IDs and explicit child order. Legacy collection
JSON defaults to schema 1. Legacy name-derived collection files retain their loaded
path; newly created collections use IDs so names cannot collide. Saving a renamed
legacy environment writes the canonical ID path and backs up retired files under
`.local/migrations` before removing duplicates. Existing corrupt files are left intact
and their load errors are shown. Writes use temporary files and atomic replacement.
Cross-collection moves save the destination first; a failed source save retains a
recoverable duplicate instead of losing the request.

## Known import omissions

Collection v2.1 and environment JSON imports now produce a structured report for
known data they cannot retain or apply: unsupported behavior settings,
malformed description/type representations and unsupported authentication configurations.

The desktop presents **Review import omissions** before saving. Cancel or close
the review to discard the pending import; choose **Import with omissions** to save
the supported subset. Changing workspaces discards a pending review.

CLI `import` and `run` reject known omissions with exit **2** unless
`--allow-lossy-import` is supplied. This also applies to an imported environment used
by `run`. Rejection does not save collection/environment data or send requests;
workspace initialization may create its manifest and empty directories first.

For `import`, `--import-report` writes a version-1 JSON report on stdout instead of
saved paths. It contains `requires_confirmation` and diagnostics with a JSON Pointer
`path`, stable `feature` code, `severity`, `disposition` and `message`. Reports do not
copy source field values, request names or response bodies. Output is limited to 256
details plus an overflow notice; overflow still requires confirmation. Check the exit
code to distinguish a saved import from rejection or a subsequent storage failure.

This flag is not a dry run: safe imports are saved normally. Malformed/unsupported
documents still fail parsing and do not produce an omission report. The report covers
the known fields listed above, not every possible extension or scripting API. Empty
reports are not a full-fidelity guarantee. Native JSON and cURL imports retain their
existing validation paths. Preserving the omitted fields remains future work.

## Variables

Precedence, from broad to narrow:

1. Collection and nested folder variables, with nearest folder overriding ancestors.
2. Active environment variables.
3. CSV/JSON iteration data.
4. Local variables and response extractions for the current send/run iteration.

`{{vault:name}}` resolves separately from the unlocked encrypted local vault. Recursive
expansion is deterministic and stops with an error on missing references or cycles.
The runner resets iteration-local state between iterations. Folder selection uses its
stable ID and runs its descendant requests in saved order. GUI runs use saved collection
requests; unsaved editor changes must be saved first.

## Synchronous script subset

QuickJS runs scripts with a 250 ms execution budget, 16 MiB memory limit and 256 KiB
stack limit. Script source is limited to 256 KiB and script response input to 1 MiB;
native assertions/extractors use the full bounded response. No filesystem, process, module loader,
network client or timers are exposed. Shared-memory blocking APIs are disabled.
Promises/asynchronous scripts are unsupported and must not be relied on.

Supported APIs:

- `pm.environment` and `pm.collectionVariables`: `get`, `set`, `unset`, `has`, `toObject`,
  `replaceIn`. Changes are in-memory during a send/run; they do not overwrite project
  environment files automatically.
- `pm.variables`: `get`, `set`, `unset`, `has`, `toObject`, `replaceIn`.
- `pm.iterationData`: the same scope accessors for the current data row. Changes do
  not persist beyond the script's data view.
- `pm.response`: `code`, `status`, `responseTime`, `json()`, `text()`,
  `headers.get(name)`, `headers.has(name)`, `to.have.status(code)`.
- `pm.test(name, fn)` and `pm.expect(value)` with chain words `to`, `be`, `have`, `and`,
  `that`, `is`; assertions `equal`, `eql`, `equals`, `include`, `contain`, `property`,
  `status`, `true`, `false`. Equality compares JSON representations.
- `console.log/warn/error` are no-ops so secrets are not written into run reports.

Unsupported `pm.*` APIs throw explicit errors, including `pm.sendRequest`,
`pm.request` mutation, `pm.globals`, `pm.vault`, `pm.execution`, external packages,
full Chai syntax, asynchronous callbacks and workflow branching. An unsupported API
inside a test produces a failed test. It is never treated as a successful no-op.
Scripts can change scoped variables to construct dynamic requests. Use serializable
assertions/extractors where possible for portable, inspectable test behavior.

## HTTP, security and generated code

TLS verification is enabled by default. An explicit per-request override is available
for development and is visibly marked in the editor. Requests support custom CA and
client-certificate PEM files, system/explicit proxies, redirect control and timeouts.
Responses are limited to 32 MiB by default (configurable up to 256 MiB), with a 1 MiB
text preview and raw download. Read failures are transport errors; total timing includes
body transfer. Cookies are scoped to the current workspace service and current session,
with inspect/set/clear controls. OAuth tokens are also session-local.

Vault passwords/keys are never stored. Local draft files contain the request text you
edit and are excluded from Git by a workspace's .gitignore. Use vault references rather
than typing secrets directly into project files. History omits request/response bodies,
redacts known sensitive headers/auth/query/form fields, and does not store raw responses.
Collection export redacts known sensitive fields including JSON secret keys by default;
**Export including credentials** is a separate explicit action. Custom names and secrets
embedded in arbitrary text cannot be reliably classified, so review exports before sharing.

Generated cURL targets POSIX shell syntax. Python uses requests; JavaScript uses Fetch
(Node 20+ for file samples); Rust uses reqwest 0.12 and Tokio. All generators resolve
query API keys, variables, supported request-level auth and body modes consistently.
Resolve inherited auth in its collection context first. OAuth configuration cannot be
exported as an already acquired token: use explicit Bearer auth for that code sample.
Browser Fetch is subject to CORS/forbidden headers and cannot configure custom TLS or
proxy settings. JavaScript samples call out runtime-specific dispatcher requirements.
The other generators include request timeout, redirect, proxy and certificate options.

## Path variables

Imported `:name` path segments use retained path-variable rows, editable in Params.
Values may use scoped `{{variables}}`; replacement encodes each value once as a path
segment. Slashes, spaces, percent signs, Unicode, query and fragment characters in
values cannot change URL structure. Missing/disabled/duplicate path variables and
`.`/`..` values are rejected before sending. Existing encoded static segments remain
encoded. Query rows are resolved and encoded separately by the shared HTTP pipeline.

## Multipart imports and generated requests

Upload arrays become repeated file rows with the same key in source order. Each
row retains its description, enabled state and MIME type and can be edited or
removed independently. Choose files adds repeated rows; an empty upload selection
remains an editable unconfigured file row. Text parts can also specify a MIME type.
Variables in MIME types and file paths resolve before sending. Disabled rows are
not opened. The encoder sets the multipart body Content-Type and boundary, replacing
an explicitly supplied body Content-Type so it cannot disagree with the payload.
The four existing snippet targets reproduce the same ordered parts and MIME types;
file-based JavaScript snippets use Node. cURL multipart snippets use a temporary
body file with cleanup, preserving binary bytes and literal text safely.

## Imported behavior settings

Supported `protocolProfileBehavior` fields are `followRedirects`, `disableCookieJar`,
`strictSSL` (booleans), and `timeout` (1–3600000 milliseconds). They inherit through
collections/folders/items with request-level overrides. Cookie-disabled requests
neither send jar cookies nor store response cookies. A false `strictSSL` requires
import review. `disableUrlEncoding: false` and `disableBodyPruning: true` match the
runtime's existing behavior. Other options remain stored as metadata and require
review because they are not applied. Known options with invalid types are rejected.
After import, the Settings tab edits the canonical request settings; reloading a
native workspace does not reapply old source metadata over those edits.

## URL and header semantics

Structured query rows control matching decoded raw query keys: disabled rows remove
those keys, enabled rows replace them in row order. Unrelated raw encoded pairs
and URL fragments remain intact. URLs can also be reconstructed from validated
protocol/host/port/path components when no raw string is supplied.

Configured header names are case insensitive. Explicit fields override generated
auth fields. Repeated ordinary headers retain order, while duplicate singleton
fields (authorization, proxy-authorization, host, content-length, content-type)
are rejected. Sending and snippets share this preparation. Python/JavaScript
map-based snippets reject repeated header fields explicitly; cURL/Rust preserve
them. Transport-generated cookies, host and content length remain runtime managed.

## Saved examples

v2.1 saved responses retain ordered headers, status, body, original request and
extension metadata. Examples are local request snapshots with exact saved bytes,
limited to 100 examples per request and 1 MiB per response. Nested examples are
rejected. The desktop can capture a response, edit/rename/duplicate/delete examples
and Try a snapshot in a separate tab without replacing its parent. Save the parent
request to persist example changes. Editing a response body replaces its bytes with
UTF-8 text. Capturing/default exporting redacts recognized sensitive fields; history
omits examples entirely. Imported examples keep source values until explicitly
redacted for export. The tree exposes saved examples as separate Try actions.

## Workspace documents

The Documents window reads and edits local UTF-8 Markdown files under documents/.
Save writes atomically; a new filename saves a copy and cannot overwrite a different
existing document. Drafts recover from .local/document-draft.json after restart.
Switching workspaces uses that workspace's own draft. Documents are limited to
1 MiB each, a flat directory and 1000 listed files. Invalid filenames and symlink
components are rejected. This editor displays Markdown source offline; rich
rendered Markdown and document synchronization are outside this delivery.

## Bulk editing and header presets

Every key/value table has a JSON bulk editor. Apply atomically replaces rows after
parsing a bounded array (1 MiB, 1000 rows); invalid JSON leaves the table unchanged.
The format retains all native row fields, ordering, duplicate keys, disabled flags
and imported metadata. Reload explicitly discards the staged JSON in favor of the
current table. Drafts are isolated by request/table context during the session.

Header presets persist in .local/header-presets.json, with explicit Append, Replace
and Delete actions. They are workspace-specific, bounded and excluded from project
Git files. Recognized credential fields require a variable reference, optionally
prefixed by Bearer or Basic, rather than a literal credential. Custom names and
metadata still require care when entering data. Presets validate header semantics;
Append may create singleton conflicts that sending reports before network I/O.

## Variable and response inspection

Inspect variables shows active values with the winning collection, nearest folder,
environment, iteration, local or vault origin. Recognized sensitive names, secret
type hints, vault values and aliases of those values are masked. Unresolved/cyclic
references display an error. Reveal is explicit and resets when the inspector opens.
The effective-header list masks all values by default, including custom names; it
uses the same variable/path/auth/header preparation as sending.

Response Body uses cached syntax highlighting with a 64 KiB display bound. Search
filters lines; JSON tree provides a JSON Pointer filter, folding, a 1000-visible-node
budget and 32-level display depth. Raw shows the bounded text preview plus 4096
original bytes as hex; Download still writes the complete bounded response. Timing
shows headers, body transfer and total, without claiming a DNS/connect/TLS breakdown.
Preview decodes PNG/JPEG only, with 8 MiB input, 4096-pixel dimensions, 4-megapixel
and decoder allocation limits. JSON uses the structural view. HTML/XML and other
text previews stay inert source: no scripts execute or remote resources load.

## Portable JSON export

Desktop collection menus provide Export v2.1 (redacted), and environment settings
provide Export environment (redacted). Existing native JSON export remains available.
CLI: rustapi export INPUT.json OUTPUT.json --format v2.1|environment|native.
The default redacts recognized credential fields; --include-credentials is explicit.
Input and output must differ. Import-review rules also apply to source documents
that contain known omissions or disable TLS checks.

Portable collections use v2.1 HTTP structure, a neutral urn:collection:v2.1.0 version
identifier, ordered items/events/rows, structured descriptions, scoped variables,
auth, settings and examples. Native assertions, extraction, script snapshots and
additional transport settings use x-rustapi version 1. Binary examples retain exact
bytes in that extension and offer a text preview in the standard body field. Other
clients may ignore the extension or require a different schema identifier; acceptance
by every external client is not claimed. Export notes disclose this boundary.
Semantic round trips are verified through RustAPI's independent import adapter;
this is not a universal format-conformance or scripting-parity claim.

## v3 HTTP collections

GUI import accepts a v3 directory, while CLI import/run accepts a directory or
collection JSON/YAML file. Export v3 directory is available from collection menus
and CLI --format v3. Export creates a new directory and never replaces an existing
one. Definition files use .resources/definition.yaml; requests use *.request.yaml;
examples use sibling <request>.resources/examples/*.example.yaml. Import also
accepts .yml, root definition.yaml and the documented resource-folder alternative.
Numeric order values sort items/examples; filename order breaks ties. Scripts are
inline JavaScript, with HTTP lifecycle mapping to the existing bounded runtime.

The supported subset covers HTTP methods, raw/JSON, form, file and multipart bodies,
headers/query/path variables, plain scope variables, supported auth, settings,
folders, scripts and examples. External secret providers, rule-selected auth arrays,
linked assets, other protocols, script package/request dependencies and external
script/include resources are rejected before persistence. Unknown resource files
are rejected rather than silently skipped. Upload references still require checking
local paths; export does not copy upload files. Rich annotations unsupported by the
native model are outside this HTTP subset.

Native-only metadata is retained in .resources/rustapi.json. It is restored only
when regenerating standard YAML from that native collection matches every parsed
YAML resource. YAML edits require explicit import review before stale native data
is omitted. Other consumers use standard YAML fields and may ignore native metadata,
including folder variables, native assertions and extra transport settings.

Limits: 1000 files, 64 MiB aggregate, 8 MiB per YAML resource, 32 directory levels,
64 YAML levels and bounded parser events/nodes/scalars. Aliases, anchors, merge keys,
unsupported tags, symlinks, ambiguous definitions and case-insensitive filename
collisions are rejected. Export uses staging plus directory rename. Metadata
resources are discovered within the selected canonical directory; includes never
read external files or make network calls.

## JSON Schema response assertions

Choose JsonSchema in Tests. Expected contains a literal JSON Schema document; an
empty Target validates the whole JSON response, otherwise Target is a JSON Pointer.
Schema assertions use the same runner in desktop sends and CLI runs and persist in
native storage and portable native extensions. Schema strings do not interpolate
variables, avoiding changes to schema structure through unescaped substitution.

The maintained validator detects the declared draft and validates formats. Remote
and filesystem retrieval features are disabled; only acyclic local JSON Pointer
references are accepted. Nested schema identifiers and anchor references are outside
this bounded subset. Regex uses a linear engine with compilation/cache size limits;
lookaround/backreference patterns are unsupported. Limits are 256 KiB of schema,
32 expansion levels, 2048 visited schema/data nodes including expanded references,
and 100000 instance nodes with 128 levels. Invalid schemas or complexity excesses
produce a failed assertion. Up to ten mismatch locations are shown without instance
or expected values. Schema complexity limits are separate from transport limits.
