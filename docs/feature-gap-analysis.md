# RustAPI feature gap analysis

Reviewed **2026-10-29** against RustAPI commit
`70b02c038d133b3e8f32cd48ccdadb51f693eddf`. This document describes RustAPI's
implemented capabilities, remaining workflow gaps and proposed product scope.

The most immediate needs are import fidelity, saved request/response examples,
editor and debugging tools, script compatibility, and API specifications/documentation.
Collaboration, advanced automation, protocol clients and governance are separate stages.

See the [implementation plan](implementation-plan.md) for dependencies and acceptance
criteria, and [implementation progress](implementation-progress.md) for the current
step. The [historical audit](local-rest-audit.md) describes an earlier baseline;
its missing-feature table is not the status of the current checkout.

Delivery update: E00 fixtures, E01.1 known-omission review and plain-text row/request/folder
description preservation are implemented. Other omitted fields still require review.

## Evidence and scope

- **Implemented** means a production code path was found, not complete feature coverage.
- **Partial** means a useful implementation exists with the listed limitations.
- **Absent** means no corresponding model/service/UI was found in the reviewed checkout.
- This is a source and existing-test review. Runtime, performance, security and release
  certification are separate checks; prior results are not new measurements.
- Collection v2.1 JSON and environment JSON imports exist. Collection v3 YAML
  directories are not supported. These external formats are separate from RustAPI's
  native workspace schema.
- Saved examples pair a request snapshot with an editable response. They need their
  own persistent model; redacted request history is not an equivalent feature.

## Existing RustAPI foundation to retain

| Capability | Evidence in this revision | Boundary |
| --- | --- | --- |
| Local project folders and persistence | [workspace.rs](../src/workspace.rs#L24), [file_storage.rs](../src/storage/file_storage.rs) | Versioned manifest, atomic writes, stable storage identities and local-state Git ignores; no account required. |
| Collections and request lifecycle | [collection.rs](../src/models/collection.rs), [tabs.rs](../src/ui/tabs.rs), [app.rs](../src/app.rs) | Nested folders, inheritance, ordering/moves, Save As, dirty tracking, close protection, drafts and per-tab cancellation/results exist. |
| Everyday HTTP requests | [request.rs](../src/models/request.rs), [client.rs](../src/http/client.rs) | Seven methods, query/header rows, JSON/text, forms, file uploads and binary bodies exist. |
| Variables and credentials | [variables.rs](../src/variables.rs), [auth.rs](../src/models/auth.rs), [oauth.rs](../src/http/oauth.rs), [secrets.rs](../src/secrets.rs) | Scoped variables, inherited auth, basic/bearer/API key, two OAuth flows, encrypted local vault and redaction exist. |
| Transport controls | [client.rs](../src/http/client.rs#L89), [workflow.rs](../src/models/workflow.rs) | Verified TLS, CA/client PEM, proxies, redirect/timeout/size controls and session cookies exist. |
| Tests and scripts | [runner.rs](../src/runner.rs#L98), [scripts.rs](../src/scripts.rs) | Native assertions/extractors and a bounded synchronous `pm.*` subset exist. |
| Runs and automation | [runner.rs](../src/runner.rs), [cli.rs](../src/cli.rs) | Shared GUI/CLI execution, CSV/JSON input, iterations, folder selection, cancellation, progress, JSON/JUnit and exit codes exist. |
| Migration and snippets | [import.rs](../src/import.rs), [code_gen.rs](../src/http/code_gen.rs) | Collection v2.1/environment and cURL import, native JSON export, four code-generation targets exist. |

Existing regression evidence is described in [validation.md](validation.md).
Its historical results are not a new measurement of this review's commit.

## Gap matrix: daily use and migration

The IDs below map to work packages in the implementation plan. **P0** means
correctness/data-preservation gate; **P1** means next daily-client work; **P2** means
later local capabilities; **P3** means a separate platform decision.

| ID | Workflow capability | RustAPI status and remaining work | Priority / package |
| --- | --- | --- | --- |
| G01 | Portable collection formats, including v3. | **Partial.** [import adapter](../src/import.rs#L238) accepts JSON and checks v2.1; no v3 YAML/directory adapter. Add clear format detection and a documented v2.1 fallback before v3 support. | P0 diagnostics; P1 adapter / E01, E04 |
| G02 | Descriptions, structured path parameters and richer multipart inputs. | **Implemented for the documented HTTP subset.** Structured descriptions, scope metadata, editable path variables, multipart arrays/MIME, inherited settings and shared URL/header semantics are delivered. Unknown behavior settings require import review. See [compatibility](import-compatibility.md). | P0 / E01 |
| G03 | Named saved examples per request, editable response and Try action. | **Implemented.** Bounded exact response bytes, original request snapshots, import, native persistence, edit/duplicate/delete and independent Try tabs. Default capture/export redacts recognized fields; history omits examples. | P1 / E01, E02 |
| G04 | Exchange collections with other clients and tooling. | **Partial.** Import exists; [export](../src/storage/file_storage.rs#L255) serializes native RustAPI collections only. No collection v2.1/environment exporter or supported round-trip contract. | P1 / E04 |
| G05 | Parameter/header descriptions, bulk editing, header presets/autocomplete and effective-header explanations. | **Partial.** Description editing, lossless JSON bulk editing and workspace-local header presets are implemented. Effective configured-header inspection is implemented with masking. Autocomplete remains. | P1 / E03 |
| G06 | Custom methods, URL/query/path editing and syntax-aware raw bodies. | **Partial.** [Method enum](../src/models/request.rs#L10) is closed; no custom methods, dedicated `:id` controls or URL/query synchronization. JSON/Text can carry raw content but lack XML/HTML modes and language editing tools. | P1 / E01, E03 |
| G07 | Variable scopes, runtime generators and interactive variable inspection. | **Partial.** [variables.rs](../src/variables.rs#L6) has collection/folder, environment, iteration, local and vault resolution. A contextual inspector shows origins, masked values, alias resolution and errors. Globals, dynamic values, hover/autocomplete and shared/local value separation remain. | P1 / E03, E05 |
| G08 | JSON/XML/HTML response modes, highlighted syntax, previews, JSONPath/XPath and visualizations. | **Partial.** [Viewer](../src/ui/response_viewer.rs#L9) has Body/Headers, JSON formatting, line filtering, copy/download and status/time/size. Syntax, JSON tree/folding/Pointer filtering, raw hex, timing and bounded PNG/JPEG previews are implemented. HTML/XML stay inert source; XPath, visualizers and schema tooling remain. | P1 basic viewer; P2 advanced preview / E03, E07 |
| G09 | Request/network and script console. | **Absent.** [console.log/warn/error](../src/scripts.rs#L29) are no-ops. Transport records headers/total time but UI exposes only total. No request trace, resolved-header inspector or DNS/connect/TLS breakdown. | P1 / E05 |
| G10 | Broader request code snippets and configuration. | **Partial.** [Four generators](../src/http/code_gen.rs#L5) exist; C#, Java, Go, Dart, raw HTTP and other library variants do not. [GUI snippets](../src/app.rs#L1558) resolve inherited auth; standalone unresolved requests are rejected. OAuth acquisition/refresh is not generated and requires an explicit bearer token. Browser Fetch cannot reproduce all TLS/proxy settings. | P1 first targets; P2 long tail / E06 |
| G11 | More authorization helpers and OAuth configuration. | **Partial.** [AuthType](../src/models/auth.rs#L8) lacks Digest, OAuth 1, JWT signing, AWS SigV4, Hawk, NTLM and vendor helpers. OAuth has client credentials/PKCE/refresh but [client secret submission](../src/http/oauth.rs#L51) is fixed to form data; no configurable extra provider parameters or token manager. | P1 OAuth; P2 other helpers / E06 |
| G12 | Cookie management and capture/sync from browser traffic. | **Partial.** [Session jar](../src/http/client.rs#L27) supports inspect/set/clear. No persistence, richer edit grid, per-request controls, browser capture or cookie sync. Forward-proxy configuration is not a capture proxy. | P2 / E06, E12 |
| G13 | JavaScript request mutation, auxiliary requests, workflow control, assertion libraries and packages. | **Partial.** [scripts.rs](../src/scripts.rs#L7) implements only scoped accessors, basic response access/tests and limited assertions. No `pm.request`, `pm.sendRequest`, `pm.execution`, `pm.globals`, `pm.vault`, packages, full Chai or async support. | P1 common subset; P2 async/packages / E05 |
| G14 | Schema/spec validation and contract workflows. | **Absent for schemas.** [AssertionKind](../src/models/workflow.rs#L42) has status/header/JSON Pointer/contains/time checks, but no JSON Schema validation or specification link. Spec linting and response validation need separate services. | P1 assertions; P2 spec integration / E05, E07 |
| G15 | Reusable datasets and richer repeatable runs. | **Partial.** [Runner](../src/runner.rs#L10) accepts CSV/JSON per run, but lacks a managed dataset library, saved run profiles, persisted run history or selected-request plans. SQL views and database sources are additional possible extensions. | P2 / E09 |
| G16 | Rich workspace navigation, overview and documents; reusable linked assets. | **Partial.** [Manifest](../src/workspace.rs#L24) contains name/version; [tree search](../src/app.rs#L1083) matches request names/URLs. The local Documents editor provides bounded Markdown files and draft recovery. No global content search, favorites, linked requests or embedded Git changes/conflict view. | P1 docs/search; P2 Git/links / E02, E03, E12 |

### Additional correctness details

The shared [header preparation](../src/http/prepared.rs) now preserves ordinary
repeated fields, rejects duplicate singleton fields and applies explicit-auth
precedence consistently. Map-based snippet targets reject unsupported repetition
instead of collapsing it. The [compatibility contract](import-compatibility.md)
documents raw-query replacement and encoder-owned multipart boundaries.

Script limits are intentional: 256 KiB source, 16 MiB heap, 250 ms execution and a
1 MiB response-input limit; see [scripts.rs:41](../src/scripts.rs#L41). Adding
compatibility must preserve bounded execution. A working QuickJS runtime does not
mean arbitrary imported scripts work. The [compatibility contract](import-compatibility.md)
lists the current supported subset.

## Gap matrix: API lifecycle, protocols and platform

For absent features, the evidence is the [library module inventory](../src/lib.rs),
[models](../src/models/mod.rs), [CLI commands](../src/cli.rs), [workspace model](../src/workspace.rs)
and [application UI](../src/app.rs), together with the specific boundaries above.

| ID | Workflow capability | RustAPI status and practical scope | Priority / package |
| --- | --- | --- | --- |
| G17 | Specifications, editing, linting, collection generation and documentation preview. | **Absent.** No OpenAPI import/editor, schema registry, spec-to-request generation or drift checking. Start with OpenAPI import and local validation. | P1 import; P2 authoring / E07 |
| G18 | Generated collection documentation and public publishing. | **Absent.** Native JSON export is not endpoint documentation. Add local Markdown/HTML generation from descriptions/examples before hosting. | P1 local; P3 hosted / E02, E07, E12 |
| G19 | Local programmable mocks and deployed public/private mock servers. | **Absent.** Start with deterministic local example-backed HTTP mocks; scripted/stateful mocks and hosted access are separate stages. | P2 local; P3 hosted / E08, E12 |
| G20 | Scheduled API monitors, history, alerts and private-network execution. | **Absent as a service.** Existing headless CLI can be invoked by an external scheduler today. No built-in scheduler, monitor dashboard, alert policy or hosted execution. | P2 local/external scheduling; P3 hosted / E09, E12 |
| G21 | Performance tests with virtual users and load profiles. | **Absent.** Sequential iterations are functional tests, not load tests. Need bounded concurrency, latency distributions, rates and explicit load controls. | P2 / E10 |
| G22 | GraphQL queries/variables and schema-assisted editing. | **Partial transport only.** JSON POST works manually; imported GraphQL mode is rejected. No query editor/introspection/schema explorer. | P2, after import diagnostics / E11 |
| G23 | WebSocket, Socket.IO, gRPC and MQTT clients. | **Absent.** The HTTP request/complete-response model is not a persistent session or typed streaming client. | P2 selected protocols; remainder deferred / E11 |
| G24 | HTTP/SSE inspection and richer protocol tools. | **Partial.** Raw SOAP XML can be sent with manual headers. Responses are buffered until completion/limit, so there is no live SSE viewer. No WSDL workflow. | P2 / E11 |
| G25 | AI-model requests and MCP servers/tools/resources/prompts. | **Absent.** Calling a model's REST endpoint manually is possible; dedicated AI/model comparison or MCP sessions are not implemented. | P3 / E12 |
| G26 | Visual executable Flows with data mapping and debugging. | **Absent.** A collection runner is not a graph editor/runtime. Start with script/run control improvements; assess a visual layer separately. | P3 / E12 |
| G27 | Shared workspaces, invitations, comments, forks, permissions and partner collaboration. | **Absent as an integrated service.** Files can already be shared through external Git. No account, sync server or team authorization service. | P3 / E12 |
| G28 | API Catalog, organizational health/ownership and public/private API discovery networks. | **Absent.** Needs service metadata, search, ingestion and access controls beyond local request collections. | P3 / E12 |
| G29 | Application Inventory and organization-wide reports/integrations. | **Absent.** No UI-test traffic capture/coverage mapping or production observability. JSON/JUnit output enables external CI integration but does not provide these services. | P3 / E12 |
| G30 | AI assistance and architectural context graphs. | **Absent.** No AI chat/actions or cross-service context graph. Any future assistant needs explicit context selection, secret handling and action boundaries. | P3 / E12 |
| G31 | External/shared secrets, enterprise administration and governance. | **Partial local secrets; absent organizational controls.** Encrypted local vault exists. No external secret-manager integrations, organization RBAC/SSO/provisioning/audit service or centralized policy. | P2 selected secret integrations; P3 organization / E12 |

SDK generation, production insights and private-network monitoring belong to optional
platform scope, distinct from adding more request snippets.

## Recommended scope

Make RustAPI dependable for **import → edit → send → inspect → test → run in CI**
first. Prioritize G01–G14 and local descriptions/examples/docs, then OpenAPI,
mocks and reusable run assets. Keep existing account-free operation, shared GUI/CLI
execution and explicit compatibility boundaries.

Add protocols and platform services according to actual workloads. The implementation
plan keeps broader platform capabilities visible without making them prerequisites
for a useful REST client.
