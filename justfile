# wed justfile
default:
    @just --list

build:
    cargo build

release:
    cargo build --release

run *args:
    cargo run -- {{ if args == "" { "src/main.rs" } else { args } }}

test:
    cargo test --workspace

test-verbose:
    cargo test --workspace -- --nocapture

lint:
    cargo clippy --workspace --all-targets -- -D warnings

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

ci: fmt-check lint test

clean:
    cargo clean
