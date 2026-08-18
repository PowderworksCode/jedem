#!/usr/bin/env bash
# Build the extension module and run a real Python process against it.
#
# This is the end-to-end proof: derive -> descriptor -> generator -> pyo3 ->
# a Python interpreter importing the result and calling it. A backend without
# one of these does not count as done.
set -euo pipefail
cd "$(dirname "$0")"

echo "== regenerating the binding"
cargo run -q -p hello --bin jedem-generate

echo "== building the extension module"
cargo build -q -p hello-python

# Cargo emits libhello.so; Python wants hello.so on the import path.
OUT="$(cargo metadata --format-version 1 --no-deps 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
mkdir -p .pyimport
cp "$OUT/debug/libhello.so" .pyimport/hello.so

echo "== running python"
PYTHONPATH=.pyimport python3 test.py
