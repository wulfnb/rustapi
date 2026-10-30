# Active delivery backlog

Requested 2026-10-30: complete import correctness, daily workflow and compatibility.
Deliver each verified slice on `main` with its own commit and push. The current
scope stops before OpenAPI, mocks, scheduling, load testing, protocol clients and
hosted collaboration. Details and evidence live in [implementation progress](implementation-progress.md).

## Import correctness

- [x] Request/folder descriptions with backward-compatible storage.
- [x] Structured descriptions and collection/folder variable metadata.
- [ ] Path-variable substitution and encoding.
- [ ] Multiple multipart files and per-part MIME types, including generated samples.
- [ ] Imported request settings and explicit unsupported-setting diagnostics.
- [ ] URL/query and header repetition/override semantics shared with snippets.

## Daily workflow

- [ ] Saved request/response examples with import, persistence, editing and Try.
- [ ] Workspace documents with offline viewing/editing.
- [ ] Request/folder/row description editing.
- [ ] Lossless bulk row editing and persistent header presets.
- [ ] Variable inspection with scope origins and masked secrets.
- [ ] Rich response inspection with syntax/tree/raw/filter/preview controls.

## Compatibility

- [ ] Collection v2.1/environment export and semantic round trips.
- [ ] v3 YAML collection-directory import/export with bounded resource resolution.
- [ ] Broader documented script support, including request mutation and workflow control.
- [ ] JSON Schema response assertions using a maintained validator.
- [ ] Bounded script/network debugging console with redaction.
- [ ] OAuth provider/client-auth options.
- [ ] Additional snippet targets: raw HTTP, Go, C#, Java and Dart.

Completed checkpoints must record exact tests and limitations. Checking an item
requires an implemented GUI/CLI path where relevant, persistence and regression
evidence; adding a schema field alone is not feature completion.
