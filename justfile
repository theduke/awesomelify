set dotenv-load

[private]
default:
  @just --list

serve:
  cargo watch -x "run -- serve"

lint: lint-fmt lint-rust lint-clippy

lint-fmt:
  cargo fmt --version
  cargo fmt --check

lint-rust:
  cargo --version
  cargo check

lint-clippy:
  cargo clippy --version
  cargo clippy -- --deny warnings

# Fix lints (rustfmt + clippy)
fix:
  cargo clippy --fix
  cargo fmt

test:
  cargo nextest run
  # nextest does not support doc tests, so run those separately
  cargo test --doc

test-ci:
  # Run nextest with slightly tweaked options to make it more suitable for CI.
  cargo nextest run --no-fail-fast --failure-output=immediate-final
  # nextest does not support doc tests, so run those separately
  cargo test --doc

# Run all lints and tests to ensure CI will pass.
prepush: lint test

deploy:
  ./infra/deploy.sh
