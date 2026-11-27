#!/usr/bin/env bash
# Installs the NSH CLI built from the commit Cargo.lock pins nsh-core to, and adds it to GITHUB_PATH.
#
# Cargo.toml follows NSH (`branch = "main"`, `tag = "nightly"`, or a release tag); Cargo.lock holds the exact commit.
# The prebuilt nightly (or release) CLI is used while that tag still points at the locked commit. `nightly` moves
# every night, so once it has moved on, the CLI is built from the locked commit instead, the way `install.sh --source`
# builds main: a checkout's CLI builds its hosts and runtimes from that same checkout.
#
# Runs on Linux, macOS and Windows (Git Bash) from the repository's root.
set -euo pipefail

repository=longbridge/nsh
lock_source="$(tr -d '\r' < Cargo.lock | grep -A2 '^name = "nsh-core"$' | sed -n 's/^source = "\(.*\)"$/\1/p')"
locked="${lock_source##*#}"
tag="$(printf '%s' "$lock_source" | sed -n 's/.*[?&]tag=\([^&#]*\).*/\1/p')"
if [ -z "$tag" ]; then
  tag="nightly"
fi
if [ -z "$lock_source" ] || ! printf '%s' "$locked" | grep -Eq '^[0-9a-f]{40}$'; then
  echo "::error::Cargo.lock does not pin nsh-core to a Git commit ($lock_source)."
  exit 1
fi

# An annotated tag lists the commit it points at as `<tag>^{}`.
tagged="$(git ls-remote "https://github.com/$repository.git" "refs/tags/$tag" "refs/tags/$tag^{}" | tail -n 1 | cut -f1)"
echo "Cargo.lock: nsh-core at $locked (tag $tag, now at ${tagged:-unknown})"

path_entry() {
  if command -v cygpath > /dev/null 2>&1; then cygpath -w "$1"; else printf '%s\n' "$1"; fi
}

if [ -n "$tagged" ] && [ "$tagged" = "$locked" ]; then
  echo "Installing the prebuilt NSH CLI of $tag"
  case "$(uname -s)" in
    MINGW* | MSYS* | CYGWIN*)
      NSH_VERSION="$tag" pwsh -NoProfile -Command 'irm https://nsh.dev/install.ps1 | iex'
      path_entry "$LOCALAPPDATA/nsh/bin" >> "$GITHUB_PATH"
      ;;
    *)
      curl -fsSL https://nsh.dev/install.sh | NSH_VERSION="$tag" sh
      echo "$HOME/.local/bin" >> "$GITHUB_PATH"
      ;;
  esac
  exit 0
fi

source="${RUNNER_TEMP:-${TMPDIR:-target/agents/tmp}}/nsh-$locked"
echo "Building the NSH CLI from $locked in $source"
if [ ! -e "$source/.git" ]; then
  git init -q "$source"
  git -C "$source" remote add origin "https://github.com/$repository.git"
fi
git -C "$source" fetch -q --depth 1 origin "$locked"
git -C "$source" checkout -q --detach FETCH_HEAD
(cd "$source" && cargo build --release --locked -p nsh-cli)
target="${CARGO_TARGET_DIR:-target}"
case "$target" in
  /*) ;;
  *) target="$source/$target" ;;
esac
path_entry "$target/release" >> "$GITHUB_PATH"
