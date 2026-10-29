# Contributing to RustAPI

Anyone is welcome to report bugs, suggest improvements, and submit pull requests.
Fork the repository, create a focused branch, and describe the behavior your change
improves. Include regression tests for fixes and new behavior.

## Development

Install Rustup and the native dependencies listed in README.md. The repository pins
its Rust toolchain. Use an isolated profile when testing the desktop:

```sh
RUSTAPI_DATA_DIR=/tmp/rustapi-development cargo run --locked
```

Before submitting a pull request:

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Coverage checks must exercise production behavior. Do not hide executable source
files, add coverage exclusions, or remove working features to satisfy the target.
See docs/ci-cd.md for platform checks and packaging validation.

## License

This revision and subsequent contributions use **GPL-3.0-or-later**. By submitting
a contribution, you agree to make it available under that same license. You retain
copyright in your contributions; no copyright assignment is required.

Commercial use and sale are allowed. When distributing a modified or combined work
covered by the GPL, distributors must provide the corresponding source and the same
GPL freedoms to recipients. Private use does not require publishing modifications.
Using RustAPI to test a separate product's API does not license that product under
the GPL. The complete terms in LICENSE control.

The license change applies to this revision onward, not retroactively to earlier
revisions that were labelled MIT. Keep third-party license notices with their code.
