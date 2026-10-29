# RustAPI compared with Postman

Reviewed **2026-10-29** against RustAPI commit
`70b02c038d133b3e8f32cd48ccdadb51f693eddf`, the seven supplied Postman screenshots,
and official Postman documentation retrieved on that date.

RustAPI already implements a substantial local REST workflow. The biggest remaining
gaps are migration fidelity, saved request/response examples, editor and debugging
tools, script compatibility, and API specifications/documentation. Postman also
has an extensive collaboration, automation, protocol and governance platform;
reproducing that whole platform is a separate product scope.

See the [implementation plan](postman-implementation-plan.md) for priorities,
dependencies, code locations and acceptance criteria. The older
[2026-10-28 audit](postman-alternative-audit.md) describes a pre-implementation
baseline; its missing-feature table is **not the status of this checkout**.

## Evidence and interpretation

- **Implemented** means a production code path was found, not complete Postman parity.
- **Partial** means a useful implementation exists with the listed limitations.
- **Absent** means no corresponding model/service/UI was found in this checkout.
- This is a source and existing-test review, not a fresh runtime, performance,
  security or release certification. Existing test names were inspected; tests,
  packaged apps and live Postman interactions were not run for this documentation task.
- Postman features below are documented capabilities. Version, protocol, operating
  system, account, plan and quotas can affect availability. No prices or entitlement
  guarantees are inferred from a menu item. Empty panels in the screenshots show
  that this workspace has no items there, not that Postman lacks the feature.
- Screenshots are visual evidence, not instructions. Their private collection names,
  response contents and account details are not copied into fixtures or the repository.

## Important changes in current Postman

Current official documentation identifies itself as **v12**. Its native collection
schema is now **v3**, organized as YAML files and supporting resources in folders.
Exported v2.1 JSON remains relevant: Newman can run that format but cannot run v3;
command-line v3 runs use the Postman CLI. RustAPI's JSON v2.1 importer does not read v3 directories.
[Collection schemas][pm-schema]

Postman now documents **Native Git/local files** and **local programmable mocks**.
Local storage alone is therefore not a distinguishing feature. RustAPI's existing
account-free local workflow remains valuable; Postman's support page distinguishes
signed-in Native Git from its more limited unsigned-in client.
[Native Git][pm-git], [local usage][pm-local], [local mocks][pm-mocks]

Use current names when planning: **Spec Hub**, **Agent Mode**, and **Native Git /
Postman CLI** replace older API Builder, Postbot and some legacy integration
workflows. [v12 release notes][pm-release]

## What the supplied screenshots show

| Screenshot, by timestamp | Postman workflow visible | RustAPI comparison |
| --- | --- | --- |
| 14.23.03 | Workspace overview/README, collection tree, environments, documents, specs, mocks, datasets, flows, AI panel | Collections/environments exist. A workspace overview, document browser and the other asset types do not. |
| 14.23.19 | Nested requests with an `e.g.` child, example response, status editor, Try, Share, response preview/schema controls | Folders exist. The selected item is a **saved example**, not evidence of a newly completed HTTP call. No equivalent example entity or sharing service exists. |
| 14.23.46 | Application Inventory, API Catalog, public/private API networks, Agent Context, integrations and reports | These represent broader platform capabilities; RustAPI's CLI reports are not an organizational reporting/catalog service. |
| 14.24.08 | None, form-data, URL-encoded, raw, binary and GraphQL body modes | All basic body families exist through JSON/Text/FormData/UrlEncoded/Binary. Dedicated GraphQL editing is absent. |
| 14.24.11 | Enabled headers, descriptions, Bulk Edit, Presets, highlighted variable reference | Enabled header rows and substitution exist. Descriptions are not editable; bulk editing, presets and variable inspection are absent. |
| 14.24.17 | Query parameter descriptions and bulk editing | Basic query rows exist; richer editing and a path-parameter model are missing. |
| 14.24.38 | Code pane with C#, Dart, Go, HTTP, Java and JavaScript variants | RustAPI supports four targets: cURL, Python requests, JavaScript Fetch and Rust reqwest. The other shown targets/settings are absent. |

Postman's example documentation confirms that **Try** opens an example's request
in another tab. A saved example pairs request data with response status, headers and
body, and can feed docs and mocks. RustAPI's deliberately redacted history is not a
substitute. [Examples][pm-examples]

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
| Migration and snippets | [import.rs](../src/import.rs), [code_gen.rs](../src/http/code_gen.rs) | Postman v2.1/environment and cURL import, native JSON export, four code-generation targets exist. |

Existing regression evidence is described in [validation.md](validation.md).
Its historical results are not a new measurement of this review's commit.

## Gap matrix: daily use and migration

The IDs below map to work packages in the implementation plan. **P0** means
correctness/data-preservation gate; **P1** means next daily-client work; **P2** means
later local capabilities; **P3** means a separate platform decision.

| ID | Postman capability and source | RustAPI status and remaining work | Priority / package |
| --- | --- | --- | --- |
| G01 | Portable collection formats, including current v3. [Schemas][pm-schema] | **Partial.** [import.rs:223](../src/import.rs#L223) accepts JSON and checks v2.1; no v3 YAML/directory adapter. Add clear format detection and a documented v2.1 fallback before v3 support. | P0 diagnostics; P1 adapter / E01, E04 |
| G02 | Descriptions, structured path parameters and richer multipart inputs. [Parameters][pm-params] | **Partial.** [import.rs:20](../src/import.rs#L20) drops row descriptions; [request adapter](../src/import.rs#L104) omits request descriptions, structured URL variables and request-specific behavior settings. Multipart file arrays retain only their first entry; per-part MIME types are unrepresented. These losses have no specific warning. | P0 / E01 |
| G03 | Named saved examples per request, editable response and Try action. [Examples][pm-examples] | **Absent.** [ApiRequest](../src/models/request.rs#L137) has no examples; importer ignores saved responses. [History](../src/models/history.rs#L63) omits bodies and raw responses. | P0 import disclosure; P1 entity / E01, E02 |
| G04 | Exchange collections with other Postman users/tooling. [Schemas][pm-schema] | **Partial.** Import exists; [export](../src/storage/file_storage.rs#L255) serializes native RustAPI collections only. No Postman collection/environment exporter or supported round-trip contract. | P1 / E04 |
| G05 | Parameter/header descriptions, bulk editing, header presets/autocomplete and effective-header explanations. [Parameters][pm-params], [headers][pm-headers] | **Partial.** [KeyValue.description](../src/models/request.rs#L114) exists but [editor](../src/ui/request_editor.rs#L117) shows only enabled/key/value/delete. No bulk editor, presets or computed-header view. | P1 / E03 |
| G06 | Custom methods, URL/query/path editing and syntax-aware raw bodies. [Requests][pm-requests], [parameters][pm-params] | **Partial.** [Method enum](../src/models/request.rs#L10) is closed; no custom methods, dedicated `:id` controls or URL/query synchronization. JSON/Text can carry raw content but lack XML/HTML modes and language editing tools. | P1 / E01, E03 |
| G07 | Variable scopes, runtime generators and interactive variable inspection. [Variables][pm-vars], [dynamic values][pm-dynamic] | **Partial.** [variables.rs](../src/variables.rs#L6) has collection/folder, environment, iteration, local and vault resolution. No globals, dynamic values, hover/autocomplete or shared/local value separation. | P1 / E03, E05 |
| G08 | JSON/XML/HTML response modes, highlighted syntax, previews, JSONPath/XPath and visualizations. [Responses][pm-responses], [sandbox][pm-sandbox] | **Partial.** [Viewer](../src/ui/response_viewer.rs#L9) has Body/Headers, JSON formatting, line filtering, copy/download and status/time/size. No tree/folding, structural filtering, rendered media/HTML, visualizer or schema UI. | P1 basic viewer; P2 advanced preview / E03, E07 |
| G09 | Request/network and script console. [Debugging][pm-debug] | **Absent.** [console.log/warn/error](../src/scripts.rs#L29) are no-ops. Transport records headers/total time but UI exposes only total. No request trace, resolved-header inspector or DNS/connect/TLS breakdown. | P1 / E05 |
| G10 | Broader request code snippets and configuration. [Code generation][pm-codegen] | **Partial.** [Four generators](../src/http/code_gen.rs#L5) exist; C#, Java, Go, Dart, raw HTTP and other library variants do not. [GUI snippets](../src/app.rs#L1513) resolve inherited auth; standalone unresolved requests are rejected. OAuth acquisition/refresh is not generated and requires an explicit bearer token. Browser Fetch cannot reproduce all TLS/proxy settings. | P1 first targets; P2 long tail / E06 |
| G11 | More authorization helpers and OAuth configuration. [Auth types][pm-auth], [OAuth 2][pm-oauth], [AWS][pm-aws] | **Partial.** [AuthType](../src/models/auth.rs#L8) lacks Digest, OAuth 1, JWT signing, AWS SigV4, Hawk, NTLM and vendor helpers. OAuth has client credentials/PKCE/refresh but [client secret submission](../src/http/oauth.rs#L51) is fixed to form data; no configurable extra provider parameters or token manager. | P1 OAuth; P2 other helpers / E06 |
| G12 | Cookie management and capture/sync from browser traffic. [Capture][pm-capture] | **Partial.** [Session jar](../src/http/client.rs#L27) supports inspect/set/clear. No persistence, richer edit grid, per-request controls, browser capture or cookie sync. Forward-proxy configuration is not a capture proxy. | P2 / E06, E12 |
| G13 | JavaScript request mutation, auxiliary requests, workflow control, assertion libraries and packages. [Sandbox][pm-sandbox] | **Partial.** [scripts.rs](../src/scripts.rs#L7) implements only scoped accessors, basic response access/tests and limited assertions. No `pm.request`, `pm.sendRequest`, `pm.execution`, `pm.globals`, `pm.vault`, packages, full Chai or async support. | P1 common subset; P2 async/packages / E05 |
| G14 | Schema/spec validation and contract workflows. [Specification validation][pm-spec-validation], [sandbox][pm-sandbox] | **Absent for schemas.** [AssertionKind](../src/models/workflow.rs#L42) has status/header/JSON Pointer/contains/time checks, but no JSON Schema validation or specification link. Spec linting and response validation need separate services. | P1 assertions; P2 spec integration / E05, E07 |
| G15 | Reusable datasets and richer repeatable runs. [Datasets][pm-data] | **Partial.** [Runner](../src/runner.rs#L10) accepts CSV/JSON per run, but lacks a managed dataset library, saved run profiles, persisted run history or selected-request plans. Postman's dataset service also supports SQL views and other sources. | P2 / E09 |
| G16 | Rich workspace navigation, overview and documents; reusable linked assets. [Native Git][pm-git], [requests][pm-requests] | **Partial.** [Manifest](../src/workspace.rs#L24) contains name/version; [tree search](../src/app.rs#L1083) matches request names/URLs. No README/document hub, global content search, favorites, linked requests or embedded Git changes/conflict view. | P1 docs/search; P2 Git/links / E02, E03, E12 |

### Additional correctness details

The header editor permits repeated names, but [HTTP execution](../src/http/client.rs#L191)
and [code generation](../src/http/code_gen.rs#L61) collapse them through map insertion.
Define and expose override/repetition semantics before extending bulk editing or
signing. This is a source-observed limitation, not proof of a Postman discrepancy:
Postman also documents overridden headers. [Headers][pm-headers]

Script limits are intentional: 256 KiB source, 16 MiB heap, 250 ms execution and a
1 MiB response-input limit; see [scripts.rs:41](../src/scripts.rs#L41). Adding
compatibility must preserve bounded execution. A working QuickJS runtime does not
mean arbitrary Postman scripts work. The [compatibility contract](postman-compatibility.md)
lists the current supported subset.

## Gap matrix: API lifecycle, protocols and platform

For absent features, the evidence is the [library module inventory](../src/lib.rs),
[models](../src/models/mod.rs), [CLI commands](../src/cli.rs), [workspace model](../src/workspace.rs)
and [application UI](../src/app.rs), together with the specific boundaries above.

| ID | Postman capability and source | RustAPI status and practical scope | Priority / package |
| --- | --- | --- | --- |
| G17 | Spec Hub: specifications, editing, linting, collection generation and documentation preview. [Spec editing][pm-spec], [validation][pm-spec-validation] | **Absent.** No OpenAPI import/editor, schema registry, spec-to-request generation or drift checking. Start with OpenAPI import and local validation. | P1 import; P2 authoring / E07 |
| G18 | Generated collection documentation and public publishing. [Publishing][pm-docs] | **Absent.** Native JSON export is not endpoint documentation. Add local Markdown/HTML generation from descriptions/examples before hosting. | P1 local; P3 hosted / E02, E07, E12 |
| G19 | Local programmable mocks and deployed public/private mock servers. [Local mocks][pm-mocks], [deployment][pm-hosted-mocks] | **Absent.** Start with deterministic local example-backed HTTP mocks; scripted/stateful mocks and hosted access are separate stages. | P2 local; P3 hosted / E08, E12 |
| G20 | Scheduled API monitors, history, alerts and private-network execution. [Monitors][pm-monitors] | **Absent as a service.** Existing headless CLI can be invoked by an external scheduler today. No built-in scheduler, monitor dashboard, alert policy or hosted execution. | P2 local/external scheduling; P3 hosted / E09, E12 |
| G21 | Performance tests with virtual users and load profiles. [Performance][pm-load] | **Absent.** Sequential iterations are functional tests, not load tests. Need bounded concurrency, latency distributions, rates and explicit load controls. | P2 / E10 |
| G22 | GraphQL queries/variables and schema-assisted editing. [GraphQL body][pm-params], [protocols][pm-requests] | **Partial transport only.** JSON POST works manually; imported GraphQL mode is rejected. No query editor/introspection/schema explorer. | P2, after import diagnostics / E11 |
| G23 | WebSocket, Socket.IO, gRPC and MQTT clients. [Protocols][pm-requests] | **Absent.** The HTTP request/complete-response model is not a persistent session or typed streaming client. | P2 selected protocols; remainder deferred / E11 |
| G24 | HTTP/SSE inspection and richer protocol tools. [API clients][pm-clients] | **Partial.** Raw SOAP XML can be sent with manual headers. Responses are buffered until completion/limit, so there is no live SSE viewer. No WSDL workflow. | P2 / E11 |
| G25 | AI-model requests and MCP servers/tools/resources/prompts. [AI features][pm-ai], [MCP client][pm-mcp] | **Absent.** Calling a model's REST endpoint manually is possible; dedicated AI/model comparison or MCP sessions are not implemented. | P3 / E12 |
| G26 | Visual executable Flows with data mapping and debugging. [Flows][pm-flows] | **Absent.** A collection runner is not a graph editor/runtime. Start with script/run control improvements; assess a visual layer separately. | P3 / E12 |
| G27 | Shared workspaces, invitations, comments, forks, permissions and partner collaboration. [Collaboration][pm-collab] | **Absent as an integrated service.** Files can already be shared through external Git. No account, sync server or team authorization service. | P3 / E12 |
| G28 | API Catalog, organizational health/ownership and public/private API discovery networks. [Catalog][pm-catalog], [platform][pm-platform] | **Absent.** Needs service metadata, search, ingestion and access controls beyond local request collections. | P3 / E12 |
| G29 | Application Inventory and organization-wide reports/integrations. [Inventory][pm-inventory], [catalog][pm-catalog] | **Absent.** No UI-test traffic capture/coverage mapping or production observability. JSON/JUnit output enables external CI integration but does not provide these services. | P3 / E12 |
| G30 | Agent Mode and architectural Agent Context/Context Graph. [Agent Mode][pm-agent], [Use Postman][pm-use] | **Absent.** No AI chat/actions or cross-service context graph. Any future assistant needs explicit context selection, secret handling and action boundaries. | P3 / E12 |
| G31 | External/shared secrets, enterprise administration and governance. [Vault][pm-use], [spec governance][pm-spec-validation], [administration][pm-admin] | **Partial local secrets; absent organizational controls.** Encrypted local vault exists. No external secret-manager integrations, organization RBAC/SSO/provisioning/audit service or centralized policy. | P2 selected secret integrations; P3 organization / E12 |

Postman's current broader product overview also advertises SDK generation,
production insights and private-network monitoring. These belong to optional
platform scope, distinct from adding more request snippets. [Platform][pm-platform]

## Recommended scope

Make RustAPI dependable for **import → edit → send → inspect → test → run in CI**
first. Prioritize G01–G14 and local descriptions/examples/docs, then OpenAPI,
mocks and reusable run assets. Keep existing account-free operation, shared GUI/CLI
execution and explicit compatibility boundaries.

Add protocols and platform services according to actual workloads. The screenshot
sidebar is useful evidence of Postman's breadth, but copying every sidebar item
would create several new products. The implementation plan therefore keeps those
items visible without making them prerequisites for a useful REST client.

## Official source index

All sources below were retrieved on **2026-10-29**. Links in the matrices identify
which capability each source supports. Source contents and plan availability can change.

- [Collection schemas][pm-schema]; [schema registry](https://schema.postman.com/).
- [Request construction][pm-requests]; [parameters and bodies][pm-params]; [headers][pm-headers].
- [Response inspection][pm-responses]; [saved examples][pm-examples]; [debugging console][pm-debug].
- [Variables][pm-vars]; [dynamic variables][pm-dynamic]; [sandbox API][pm-sandbox].
- [Authorization types][pm-auth]; [OAuth 2 configuration][pm-oauth]; [AWS Signature][pm-aws].
- [Code snippets][pm-codegen]; [traffic/cookie capture][pm-capture]; [API clients][pm-clients].
- [Native Git][pm-git]; [local usage/account boundaries][pm-local]; [v12 release notes][pm-release].
- [Spec Hub editor][pm-spec]; [spec validation][pm-spec-validation]; [documentation publishing][pm-docs].
- [Local mocks][pm-mocks]; [deployed mocks][pm-hosted-mocks]; [datasets][pm-data].
- [Monitors][pm-monitors]; [performance tests][pm-load]; [Flows][pm-flows].
- [Collaboration][pm-collab]; [API Catalog][pm-catalog]; [Application Inventory][pm-inventory].
- [Administration, identity management and audit controls][pm-admin].
- [Agent Mode][pm-agent]; [AI features][pm-ai]; [MCP client][pm-mcp]; [Context Graph/Vault overview][pm-use]; [platform overview][pm-platform].

[pm-schema]: https://learning.postman.com/docs/use/use-collections/collections-schemas
[pm-requests]: https://learning.postman.com/docs/use/send-requests/create-requests/request-basics/
[pm-params]: https://learning.postman.com/docs/use/send-requests/create-requests/parameters/
[pm-headers]: https://learning.postman.com/docs/use/send-requests/create-requests/headers/
[pm-responses]: https://learning.postman.com/docs/use/send-requests/response-data/responses/
[pm-examples]: https://learning.postman.com/docs/use/send-requests/response-data/examples
[pm-debug]: https://learning.postman.com/docs/use/send-requests/response-data/troubleshooting-api-requests
[pm-vars]: https://learning.postman.com/latest-v-12/docs/use/send-requests/variables/use-variables
[pm-dynamic]: https://learning.postman.com/docs/tests-and-scripts/write-scripts/variables-list
[pm-sandbox]: https://learning.postman.com/docs/tests-and-scripts/write-scripts/postman-sandbox-reference/overview/
[pm-auth]: https://learning.postman.com/docs/use/send-requests/authorization/authorization-types/
[pm-oauth]: https://learning.postman.com/docs/use/send-requests/authorization/oauth-20
[pm-aws]: https://learning.postman.com/docs/use/send-requests/authorization/aws-signature
[pm-codegen]: https://learning.postman.com/docs/use/send-requests/create-requests/generate-code-snippets
[pm-capture]: https://learning.postman.com/docs/use/capturing-request-data/capture-overview
[pm-clients]: https://www.postman.com/tools/api-clients
[pm-git]: https://learning.postman.com/latest-v-12/docs/use/native-git/sync
[pm-local]: https://support.postman.com/hc/en-us/articles/115003356485-Can-I-use-Postman-locally
[pm-release]: https://www.postman.com/release-notes/postman-app/v12/
[pm-spec]: https://learning.postman.com/docs/design-apis/specifications/edit-a-specification
[pm-spec-validation]: https://learning.postman.com/docs/design-apis/specifications/validate-a-specification/
[pm-docs]: https://learning.postman.com/docs/publishing-your-api/publishing-your-docs
[pm-mocks]: https://learning.postman.com/latest-v-12/docs/design-apis/mock-apis/local-mock-servers
[pm-hosted-mocks]: https://learning.postman.com/docs/design-apis/mock-apis/set-up-mock-servers
[pm-data]: https://learning.postman.com/docs/tests-and-scripts/datasets/overview
[pm-monitors]: https://learning.postman.com/docs/monitoring-your-api/intro-monitors/
[pm-load]: https://learning.postman.com/docs/tests-and-scripts/performance-testing/performance-test-configuration
[pm-flows]: https://www.postman.com/product/flows/
[pm-collab]: https://learning.postman.com/docs/collaborating-in-postman/collaboration-concepts
[pm-catalog]: https://www.postman.com/product/api-catalog/
[pm-inventory]: https://learning.postman.com/docs/tests-and-scripts/test-applications/overview/
[pm-agent]: https://learning.postman.com/docs/getting-started/basics/about-agent-mode
[pm-ai]: https://learning.postman.com/docs/getting-started/basics/about-ai/
[pm-mcp]: https://learning.postman.com/docs/use/send-requests/protocols/mcp-requests/create
[pm-use]: https://learning.postman.com/docs/use/overview
[pm-platform]: https://www.postman.com/new-postman/
[pm-admin]: https://learning.postman.com/docs/administration/admin-overview
