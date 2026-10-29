# RustAPI implementation progress

Updated: **2026-10-29**. Source baseline for the current step: `cb8c870`.

## Working agreement

- Implement one small, reviewable slice at a time.
- Run the checks appropriate to that slice before starting the next one.
- Record changed files, results, unresolved issues and the next action here so work
  can resume without depending on chat context.
- Keep completed work distinct from the proposed [implementation plan](implementation-plan.md).
- Commit and push each verified step to the same branch (`main`) before starting
  the next step, as requested on 2026-10-29.

## Step 1 — Independent product wording

Status: **complete**, verified locally. Delivery is recorded in Git history.

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

Status: **complete**, verified locally. Step 1 was pushed to `origin/main` as `966ecd0`.

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

## Next step — Step 3: E01.1 structured import diagnostics

Add a report for known omitted fields with JSON-pointer locations and no field values.
Show it before GUI persistence and require explicit continuation for lossy imports.
Expose the same report and rejection policy in the CLI. Reuse the metadata fixture
and prove cancellation/rejection leave imported project data unwritten.

Preserving descriptions, path-variable behavior and multipart semantics remains later
E01 work. E02–E12 are pending; do not begin the whole roadmap in one change.
