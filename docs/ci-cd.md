# RustAPI CI/CD

See [the local validation record](validation.md) for tested platforms and remaining
GitHub-runner validation.

## Source comparison

Checked on 2026-10-28: this checkout, `git@github.com:wulfnb/rustapi.git` (the existing
origin), and `https://github.com/rust-projs/rustapi.git` all have the same `main` tip:
`33dda88e283430d24016db1762503dc69bb96d14` (`latest code`, 2025-12-18).
Neither remote contains a newer branch tip. The remotes were inspected without changing
the local origin. Push these changes to whichever repository will own future releases;
workflow image names derive from that repository automatically.

## Validation

`.github/workflows/ci.yml` runs for pull requests, non-main branch pushes, and manual
dispatch. CD calls it as a reusable workflow for main and version tags, avoiding a
second independent CI run on main pushes.

| Job | Checks |
| --- | --- |
| Native matrix | Ubuntu 24.04 x64, macOS 15 ARM64 and Intel, Windows Server 2022 x64 |
| Native checks | Pinned Rust, formatting, Clippy with warnings denied, unit/integration tests, docs, application build, real GUI event-loop smoke test |
| Docker | Tests inside Debian Linux, production image build, Compose validation, real app window, file-chooser portal, noVNC HTTP and WebSocket, non-root user, restart and persistent volume |
| Quality | Actionlint, ShellCheck, Python syntax, LLVM line coverage uploaded as LCOV |

Coverage is measured across all Rust targets. There is no claimed coverage percentage
or artificial 100% threshold; the report exposes untested GUI paths as well as core
logic. Startup smoke checks exercise initialization and rendering, not every user
interaction. Windows uses eframe's WGPU renderer with Direct3D rather than requiring
an installed OpenGL driver. Linux uses Mesa software rendering under Xvfb in CI.

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

To reproduce the coverage job (requires `llvm-tools-preview`):

```sh
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --version 0.9.1 --locked
mkdir -p coverage
cargo llvm-cov --locked --all-targets --lcov --output-path coverage/lcov.info
```

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
