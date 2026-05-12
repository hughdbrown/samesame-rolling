default:
    @just --list

build:
    cargo build

release:
    cargo build --release

check:
    cargo check

test:
    cargo test

clippy:
    cargo clippy -- -D warnings

fmt:
    cargo fmt

fmt-check:
    cargo fmt --check

lint: fmt-check clippy

ci: fmt-check clippy test

clean:
    cargo clean

run *ARGS:
    cargo run -- {{ARGS}}

install: release
    cp target/release/samesame /usr/local/bin/.

publish-check:
    cargo check --release
    cargo clippy --release -- -D warnings
    cargo fmt --check
    cargo test --release
    cargo publish --dry-run
    cargo package

publish: publish-check
    cargo publish

tag VERSION:
    git tag -a v{{VERSION}} -m "Release {{VERSION}}"
    git push origin v{{VERSION}}
