#!/usr/bin/env bash
# Stand up a fresh jedem checkout: hooks, the workspace, and the cargo-jedem
# subcommand the round-trip demos are driven through. Safe to re-run; every step
# is idempotent.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

# A clone runs no hooks until it is pointed at them: core.hooksPath is per-clone
# configuration, so nothing a checkout carries can set it for you.
git config core.hooksPath .githooks
if [ ! -d .githooks ]; then
    echo "note: .githooks is fleet-managed and not synced here yet; git will"
    echo "      start using it the moment ordnung writes it."
fi

if ! command -v cargo >/dev/null; then
    echo "error: cargo is not on PATH; install Rust from https://rustup.rs" >&2
    exit 1
fi

echo "== workspace"
cargo build --all-targets

echo "== test"
cargo test --all

# The demos are run as `cargo jedem`, not `cargo run -p cargo-jedem`, so the
# subcommand has to be on PATH before either of them will work.
echo "== cargo-jedem"
cargo install --path crates/cargo-jedem

# A generated binding is only worth anything if a real host process can import
# it, so the round trips are checked against whichever runtimes are installed
# rather than skipped wholesale. Missing one is a gap in this machine, not a
# failure of the checkout.
for demo in python node; do
    case "$demo" in
    python) runtime=python3 flag=--python script=test.py ;;
    node) runtime=node flag=--node script=test.mjs ;;
    esac
    if command -v "$runtime" >/dev/null; then
        echo "== demo/$demo"
        (cd "demo/$demo" && cargo jedem run "$flag" "$script")
    else
        echo "== demo/$demo: skipped, $runtime is not installed"
    fi
done

echo
echo "ready. the gate this repository runs in CI:"
echo "  cargo fmt --all -- --check"
echo "  cargo clippy --all-targets -- -D warnings"
echo "  cargo test --all"
