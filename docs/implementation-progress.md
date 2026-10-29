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

## Next step — Step 2: E00 baseline and import fixture inventory

1. Inspect the existing import fixtures and record supported/unsupported cases.
2. Run the relevant baseline checks on the current revision and record their scope.
3. Define the first E01 slice: structured import diagnostics and fidelity fixtures.
   Keep metadata preservation, path-variable behavior and multipart changes as separate
   later slices, each with its own regression checks.

Remaining packages E01–E12 are pending; consult the implementation plan for their
dependencies and acceptance criteria. Do not begin the whole roadmap in one change.
