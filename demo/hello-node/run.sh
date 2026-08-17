#!/usr/bin/env bash
# Build the native addon and run a real Node process against it.
set -euo pipefail
cd "$(dirname "$0")"

echo "== regenerating the binding"
cargo run -q -p hello --bin generate

echo "== building the addon"
cargo build -q -p hello-node

# Cargo emits libhello_node.so; Node loads a file named *.node.
OUT="$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
mkdir -p .nodeimport
cp "$OUT/debug/libhello_node.so" .nodeimport/hello.node

echo "== running node"
node test.mjs
