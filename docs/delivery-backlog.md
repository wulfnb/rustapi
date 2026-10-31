# Active delivery backlog

Updated 2026-10-31: implement the remaining feature groups 6–15 after the
completed editor, variables, navigation, response and scripting work. Collaboration
and enterprise capabilities use an optional self-hosted server; local use remains
account-free. Deliver bounded, verified slices. Details and evidence live in
[implementation progress](implementation-progress.md).

## Remaining feature groups

- [x] Authentication: JWT, AWS SigV4, Digest and encrypted persistent OAuth management.
- [x] Cookies: persistent jars, structured editing, browser capture/synchronization.
- [x] Interoperability: broader v3 resources and independent compatible-client validation.
- [x] OpenAPI: import, specification editing/linting, response contract checks and drift detection.
- [x] Documentation: offline Markdown/HTML generation and optional publishing.
- [x] Mock servers: examples, matching rules, delays and simulated errors.
- [x] Automation: saved profiles/datasets, history/comparison, scheduling and alerts.
- [x] Performance: concurrent virtual users, load profiles and percentiles.
- [x] Protocols: GraphQL, live SSE, WebSocket, gRPC, MQTT and Socket.IO.
- [x] Optional platform: self-hosted collaboration, catalogs, flows, AI/MCP, secret managers and workspace/server governance controls.

These checkpoints cover the supported, bounded implementations in the
[advanced tools guide](advanced-tools.md), not complete parity with every client,
OpenAPI construct, protocol extension or enterprise identity system. Exact local
evidence and remaining platform validation are in
[remaining-feature validation](validation-remaining-features.md). Delivery is tracked
in Git history and Actions; this checklist does not imply a new published release.

## Import correctness

- [x] Request/folder descriptions with backward-compatible storage.
- [x] Structured descriptions and collection/folder variable metadata.
- [x] Path-variable substitution and encoding.
- [x] Multiple multipart files and per-part MIME types, including generated samples.
- [x] Imported request settings and explicit unsupported-setting diagnostics.
- [x] URL/query and header repetition/override semantics shared with snippets.

## Daily workflow

- [x] Saved request/response examples with import, persistence, editing and Try.
- [x] Workspace documents with offline viewing/editing.
- [x] Request/folder/row description editing.
- [x] Lossless bulk row editing and persistent header presets.
- [x] Variable inspection with scope origins and masked secrets.
- [x] Rich response inspection with syntax/tree/raw/filter/preview controls.

## Compatibility

- [x] Collection v2.1/environment export and semantic round trips.
- [x] v3 YAML collection-directory import/export with bounded resource resolution.
- [x] Broader documented script support, including request mutation and workflow control.
- [x] JSON Schema response assertions using a maintained validator.
- [x] Bounded script/network debugging console with redaction.
- [x] OAuth provider/client-auth options.
- [x] Additional snippet targets: raw HTTP, Go, C#, Java and Dart.

Completed checkpoints must record exact tests and limitations. Checking an item
requires an implemented GUI/CLI path where relevant, persistence and regression
evidence; adding a schema field alone is not feature completion.
