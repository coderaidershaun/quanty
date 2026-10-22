#!/bin/sh
set -eu
cargo check -p gui --lib --target wasm32-unknown-unknown
cargo clippy -p gui --lib --target wasm32-unknown-unknown -- -D warnings
