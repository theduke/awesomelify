set dotenv-load

[private]
default:
  @just --list

serve:
  cargo watch -x "run -- serve"

build-debug:
  cargo build

build-release:
  cargo build --release

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
prepush: lint test-ci

# Build the Docker image through the nix flake, and push the image to the
# production flag.
# This is equivalent to "deploy", since the server will automaitcally switch
# to the current production tag.
deploy:
  #!/usr/bin/env bash
  set -Eeuxo pipefail

  TAG_PROD="theduke/awesomelify:production"

  echo "Building docker image..."
  nix build .#dockerImage
  echo "Image built ... loading image"
  IMAGE=$(docker load -i ./result --quiet | awk '{ print $3 }')
  echo "Loaded image: $IMAGE"
  echo "Tagging production..."
  docker tag "$IMAGE" $TAG_PROD
  echo "Pushing image..."
  docker push $TAG_PROD
  echo "Tag $TAG_PROD pushed!"

