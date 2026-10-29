# RustAPI CI/CD

See [the validation record](validation.md) for measured coverage and hosted CI/CD results.

## Source comparison

Checked on 2026-10-28: this checkout, `git@github.com:wulfnb/rustapi.git` (the existing
origin), and `https://github.com/rust-projs/rustapi.git` all have the same `main` tip:
`33dda88e283430d24016db1762503dc69bb96d14` (`latest code`, 2025-12-18).
This established the common starting point before the CI/CD changes. Development and
release publishing now use `wulfnb/rustapi`; the local origin remains unchanged.
Workflow image names derive from the repository that runs them.

## Validation

`.github/workflows/ci.yml` runs for pull requests and manual
dispatch. CD calls it as a reusable workflow for main and version tags, avoiding a
second independent CI run on main pushes.

| Job | Checks |
| --- | --- |
| Native matrix | Ubuntu 24.04 x64, macOS 15 ARM64 and Intel, Windows Server 2022 x64 |
| Native checks | Pinned Rust, formatting, Clippy with warnings denied, unit/integration tests, docs, application build, real GUI event-loop smoke test |
| Docker | Tests inside Debian Linux, production image build, Compose validation, real app window, file-chooser portal, noVNC HTTP and WebSocket, non-root user, restart and persistent volume |
| Quality | Actionlint, ShellCheck, Rust line/function coverage, Python statement/branch coverage, and shell execution coverage |

The Linux quality job includes all production Rust source, including GUI, native
startup/shutdown, and actual import/export file dialogs. It requires **100% line and
function coverage**, without source exclusions. It also requires **100% Python
statement and branch coverage** for the release and Docker tooling and execution of
every executable shell-launcher line. Test code and third-party dependencies are
outside these production-source measurements. Stable Rust LLVM instrumentation does
not measure branch coverage; line/function coverage does not prove every possible
input, platform-specific backend, or error combination works.

Behavioral egui tests drive pointer/keyboard events and inspect accessibility output.
The native Linux test drives real GTK portal dialogs under Xvfb and Openbox, checking
import persistence, preservation of the input file, export, cancellation, and shutdown.
Coverage JSON, LCOV, screenshots, and logs are uploaded even when a check fails.
Windows uses eframe's WGPU renderer with Direct3D rather than requiring an installed
OpenGL driver. Linux uses Mesa software rendering under Xvfb in CI.

Tests cover request duplication and serialization, auth encodings and locations,
environment variables, bounded history, Unicode display, real HTTP methods and request
bodies, headers/query encoding, code-generation regressions, storage round trips and
invalid files, and tab selection/closing. Storage tests never use your normal profile.

The Docker build has separate `test`, `release`, and `runtime` stages. The test target
is run explicitly in CI; a normal image build does not implicitly run tests.

```sh
docker build --target test -t rustapi:test .
docker build -t rustapi:local .
python3 scripts/docker-smoke.py rustapi:local
```

For local Linux development on Debian/Ubuntu:

```sh
sudo apt-get update
xargs -a docker/build-packages.txt sudo apt-get install -y --no-install-recommends
sudo apt-get install -y xvfb libgl1-mesa-dri
```

To reproduce the Linux coverage job:

```sh
xargs -a docker/coverage-packages.txt sudo apt-get install -y --no-install-recommends
python3 -m venv .venv
. .venv/bin/activate
python -m pip install -r requirements-dev.txt
RUSTAPI_COVERAGE_DIR=coverage python -m coverage run -m unittest discover -s tests/python -v
python -m coverage report
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --version 0.9.1 --locked
cargo llvm-cov clean --workspace
cargo llvm-cov --locked --all-targets --no-report
export LIBGL_ALWAYS_SOFTWARE=1 XDG_CURRENT_DESKTOP=GNOME
export XDG_RUNTIME_DIR="$(mktemp -d)"
chmod 700 "$XDG_RUNTIME_DIR"
export LLVM_PROFILE_FILE="$PWD/target/llvm-cov-target/native-%p-%m.profraw"
xvfb-run -a -s '-screen 0 1800x1200x24' dbus-run-session -- \
  python tests/native_desktop.py target/llvm-cov-target/debug/rustapi
xvfb-run -a -s '-screen 0 1800x1200x24' dbus-run-session -- \
  python scripts/desktop-smoke.py target/llvm-cov-target/debug/rustapi
cargo llvm-cov report --lcov --output-path coverage/lcov.info
cargo llvm-cov report --fail-under-lines 100 --fail-under-functions 100
```

`docker/coverage.Dockerfile` provides the same dependencies for running Linux coverage
on another host. Build it with `docker build -f docker/coverage.Dockerfile -t
rustapi:coverage-tools .`, then run it with `--init`, mount the checkout at `/source`, and use a Docker volume
for `/source/target`. This quality image is separate from the shipped desktop image.

## Publishing

`.github/workflows/cd.yml` runs on pushes to main, `v*` tags, or a manual dispatch
on main/a version tag. The workflow requires full CI, validates version identity,
builds and launches applications extracted from their actual release archives,
tests the exact Docker image before pushing, then publishes downloads.

| Ref | GitHub release | GHCR tags |
| --- | --- | --- |
| main | `preview-<12-character commit>`, prerelease | `edge`, `sha-<full commit>` |
| `v0.1.0` matching Cargo.toml | `v0.1.0`, stable release | `v0.1.0`, `latest`, `sha-<full commit>` |
| version with prerelease suffix | matching prerelease tag | version tag and commit tag; no `latest` promotion |

Images are named `ghcr.io/<lowercase-owner>/<lowercase-repository>`. Docker publication
currently targets **linux/amd64** on GitHub's Linux runner. Local Docker builds also
work on ARM64; no published multi-architecture manifest is claimed.

Release assets:

- `RustAPI-macos-arm64.zip` and `.dmg`
- `RustAPI-macos-x86_64.zip` and `.dmg`
- `RustAPI-linux-x86_64.tar.gz`
- `RustAPI-windows-x86_64.zip`
- `SHA256SUMS.txt`

macOS apps are ad-hoc signed and verified. They are **not Apple Developer ID signed or
notarized**. The Linux archive is built on Ubuntu 24.04 and needs a compatible glibc,
OpenSSL 3, and desktop graphics libraries; Docker supplies its own Debian runtime.
Windows ZIPs are portable executables, not installers or Authenticode-signed packages.

Public-repository release files and container digests receive GitHub build-provenance
attestations. Private repositories skip attestations by default; set the repository
variable `ENABLE_PRIVATE_ATTESTATIONS=true` when using GitHub Enterprise Cloud.
See [GitHub's attestation availability](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations).
No assets or channel tags are published from pull-request workflows.
Published GitHub releases are immutable to workflow reruns; an interrupted draft may
be retried. Choose a new commit/version rather than overwriting a published release.

## Repository setup

1. Enable GitHub Actions in the chosen repository, with permission to use the pinned
   actions and the hosted runner labels in these workflows.
2. Permit `GITHUB_TOKEN` to write repository releases, GHCR packages, and attestations.
   Jobs request these permissions explicitly; no personal access token is required.
   Organization package policies can still restrict creation of a new GHCR package.
3. Configure branch protection to require the four Native checks, Docker checks,
   and Quality check from the CI workflow. Branch protection is a repository setting
   and is not configured by these source changes.
4. For a version release, update `Cargo.toml` and `Cargo.lock`, merge the change,
   then push a tag equal to `v` plus the package version. A mismatched tag fails.

Each action is pinned by commit SHA. Dependabot checks Actions, Cargo, and Docker
dependencies weekly. Rust updates should keep `rust-toolchain.toml` and the Docker
builder version in sync. The tracked Cargo.lock is used with `--locked` throughout.

The workflow publishes releases and images; deployment to a server is a separate
operation and needs a destination and deployment credentials.

Generated Rust samples are built as a temporary Cargo project using the repository's
locked dependency graph and target cache. This preserves platform-native linker
configuration, including the MSVC Windows SDK import-library paths. Python coverage
includes command failure, cleanup, POSIX and Windows execution paths for this driver.
