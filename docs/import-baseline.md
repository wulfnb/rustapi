# Import baseline and fixture inventory

Prepared 2026-10-29 after cleanup commit `966ecd0`, on macOS ARM64 with Rust 1.98.0.
This checkpoint adds fixtures and baseline tests without changing import behavior.

| Fixture | Classification | Expected behavior / next work |
| --- | --- | --- |
| [Nested collection](../tests/fixtures/collection-v2.1.json) | Supported subset | Retain nested ordering, inherited auth/variables, scripts and enabled flags. Existing workflow tests cover this. |
| [Body modes](../tests/fixtures/import/body-modes.json) | Supported subset | JSON, raw text, URL-encoded, single-file multipart, binary file references and no body; retain disabled rows. Files are references only, not opened during import. |
| [Environment](../tests/fixtures/import/environment.json) | Supported subset | Preserve variable values and disabled flags; scalar values become strings. |
| [Metadata gaps](../tests/fixtures/import/metadata-gaps.json) | Known omissions, not a fidelity pass | Request/folder/row descriptions, URL path variables, saved examples, request settings, additional multipart files and part MIME types need exact diagnostics before persistence. This fixture is input for step 3, not an assertion that data loss is correct. |
| [Unsupported YAML](../tests/fixtures/import/unsupported-v3.yaml) | Rejected boundary | Synthetic YAML input, not a complete external v3 export. Must fail without saving collection/environment data. A real v3 adapter/conformance corpus remains E04 work. |
| Variants generated from body fixture | Rejected boundary | v1/v2.0/v3 schema markers and unknown/GraphQL body modes fail with exit 2. Existing JSON parser cannot ingest YAML. |

The inputs use reserved test domains and fabricated data. They do not contact those
domains, open referenced upload files or contain account credentials. The fixture
schema URI is a test identifier, not a schema downloaded by the importer.

## Baseline checks

[import_fixtures.rs](../tests/import_fixtures.rs) verifies supported conversion,
managed-copy CLI imports, unchanged source bytes and rejected imports that write no
collection/environment files. Workspace initialization may create its manifest and
empty storage directories before a rejected CLI import; transactional checks refer
to imported project data, not absence of all filesystem activity.

Existing workflow, transport, workspace and CLI suites cover request execution,
storage and cancellation. The local all-target suite passed 121 tests; formatting and warnings-denied Clippy
also passed. Detailed results for this checkpoint are recorded in
[implementation progress](implementation-progress.md). Full cross-platform packages,
native GUI interaction and current coverage percentages are separate release gates.

## First implementation slice

Step 3 adds structured, value-free diagnostics for the known omitted fields, presents
them before saving, and requires explicit continuation for lossy imports. CLI output
must be machine-readable when requested. Keep field preservation, path substitution
and multipart execution changes in later commits; diagnostics must not imply those
features are implemented.
