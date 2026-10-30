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
| Generated SDK requests | Existing four-language samples on the native matrix; additional Go/.NET 8/Java/Dart and raw HTTP transport-policy checks on Linux, with pinned Dart 3.13.7 |
| Docker | Tests inside Debian Linux, production image build, Compose validation, real app window, file-chooser portal, noVNC HTTP and WebSocket, non-root user, restart and persistent volume |
| Quality | Actionlint, ShellCheck, Rust line/function coverage, Python statement/branch coverage, and shell execution coverage |

The Linux quality job includes all production Rust source, including GUI, native
startup/shutdown, and actual import/export file dialogs. It requires **75% line and
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
cargo llvm-cov report --fail-under-lines 75 --fail-under-functions 75
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
- `RustAPI-windows-x86_64.zip` and `-Setup.exe` (Inno Setup installer)
- `SHA256SUMS.txt`
- `THIRD-PARTY-NOTICES.html` bundled in every release and app bundle

Windows packaging installs a pinned Inno Setup 6.7.1 compiler from its official
release, verifies its SHA256, and requires the resulting Setup EXE. Missing installers
fail publication instead of leaving an advertised download absent. Checksums include
all seven native distributions.

### Website downloads

Main builds are GitHub prereleases. GitHub's `releases/latest/download/...` URLs
only resolve stable releases, so the website uses explicit release asset URLs.
`site/site.js` selects the most recently published complete release (preview or
stable) from the public GitHub API. It requires all seven distributions and the
checksum file to be uploaded and nonempty, and keeps downloads and checksums on
the same release. Drafts and incomplete releases are ignored. The HTML and README
pin `preview-887ded7d5f0b` as a complete fallback; the HTML links keep working when
JavaScript is disabled or the API is unavailable or rate-limited. Update that pin
when retiring an old release. `node --test tests/site_tests.mjs` covers selection,
incomplete assets, stable/preview releases and fallback behavior.

The website's Docker quickstart uses `edge`, port 6080 and `/data`, matching the
runtime image. Stable tags additionally publish `latest`. Native download fixes do
not promote a preview to a stable release.

Download recovery validation on 2026-10-30:

- All eight pinned preview asset URLs (seven packages and checksums) returned HTTP 200.
- The website resolver selected the complete preview using real GitHub release metadata.
- All 12 website tests and Actionlint passed.
- All 15 Python tooling tests passed, retaining 100% statement and branch coverage.
- The code-generation smoke test executed and validated 24 real loopback HTTP requests.
- The Docker test target compiled and passed all 205 Rust tests on Linux ARM64.

The [failed main CD run](https://github.com/wulfnb/rustapi/actions/runs/41094830748)
identified the missing Docker icon and Windows shell-program execution failures.
This validation is local; the Windows compiler/installer and full publication must
also pass hosted CD after these changes reach main.

macOS apps are ad-hoc signed and verified. They are **not Apple Developer ID signed or
notarized**. The Linux archive is built on Ubuntu 24.04 and needs a compatible glibc,
OpenSSL 3, and desktop graphics libraries; Docker supplies its own Debian runtime.
Windows distributions include both a portable ZIP and an Inno Setup installer; they are not Authenticode-signed.

Public-repository release files and container digests receive GitHub build-provenance
attestations. Private repositories skip attestations by default; set the repository
variable `ENABLE_PRIVATE_ATTESTATIONS=true` when using GitHub Enterprise Cloud.
See [GitHub's attestation availability](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations).
No assets or channel tags are published from pull-request workflows.
Published GitHub releases are immutable to workflow reruns; an interrupted draft may
be retried. Choose a new commit/version rather than overwriting a published release.

## Future Plans & Roadmap

### 1. Multi-Architecture Docker Images (`linux/amd64` and `linux/arm64`)
* **Current status**: GitHub Actions hosted Linux runners build `linux/amd64` images. Local Docker builds natively support both ARM64 and AMD64 hosts (`docker build -t rustapi:local .`).
* **Planned implementation**:
  1. Once GitHub Actions hosted ARM64 Linux runners (`ubuntu-24.04-arm64`) are enabled for open-source workflows, split the Docker builder into a parallel matrix job for both architectures.
  2. Assemble a unified multi-architecture OCI manifest using `docker buildx imagetools create` so pulling `ghcr.io/wulfnb/rustapi:latest` automatically resolves the native architecture on Raspberry Pi, AWS Graviton, and Apple Silicon Docker environments.
  3. Run the container smoke test matrix across both architectures before release promotion.

### 2. Code Signing & Distribution Integrity
* **macOS Notarization**: Requires an active Apple Developer Program enrollment (\$99/year) to sign with a Developer ID Application certificate and submit binaries to Apple's `notarytool` automated notarization service.
* **Windows Authenticode Signing**: Eliminating Microsoft Defender SmartScreen warnings ("Windows protected your PC") requires signing the executable and installer with a commercial Authenticode certificate (Standard or EV). Industry certificate authorities (DigiCert, Sectigo, SSL.com) require identity verification and hardware token/cloud HSM storage, typically costing \$200–\$450/year. Free alternatives for qualifying open-source projects include [SignPath.io](https://about.signpath.io/) or self-signing with an imported root CA.

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
