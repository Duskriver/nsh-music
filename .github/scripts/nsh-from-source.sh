#!/usr/bin/env bash
# Builds the nsh CLI from the NSH commit Cargo.lock holds and puts it on the job's PATH. The clone keeps its .git,
# so the CLI reports that commit (`nsh --version`), as a nightly does.
set -euo pipefail
rev="$(tr -d '\r' < Cargo.lock | grep -A2 '^name = "nsh-core"$' | sed -n 's/^source = ".*#\([0-9a-f]*\)"$/\1/p')"
[ -n "$rev" ] || { echo "::error::Cargo.lock holds no NSH commit."; exit 1; }
src="$RUNNER_TEMP/nsh-src"
git clone --quiet --filter=blob:none https://github.com/longbridge/nsh "$src"
git -C "$src" checkout --quiet "$rev"
# Run from the app's directory, so the app's toolchain builds it rather than NSH's rust-toolchain.toml.
cargo install --locked --path "$src/crates/nsh-cli" --root "$RUNNER_TEMP/nsh-cli"
echo "$RUNNER_TEMP/nsh-cli/bin" >> "$GITHUB_PATH"
"$RUNNER_TEMP/nsh-cli/bin/nsh" --version
