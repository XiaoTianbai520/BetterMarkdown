#!/usr/bin/env bash
#
# Cut a new release: bumps version in tauri.conf.json + package.json +
# Cargo.toml (app), commits, tags, and pushes.
#
# Usage: ./scripts/release.sh 0.2.0

set -euo pipefail

if [ $# -lt 1 ]; then
  echo "Usage: $0 <version>" >&2
  echo "Example: $0 0.2.0" >&2
  exit 1
fi

VERSION="$1"
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+ ]]; then
  echo "ERROR: version must be semver, e.g. 0.2.0" >&2
  exit 1
fi

cd "$(dirname "$0")/.."

if [ -n "$(git status --porcelain)" ]; then
  echo "ERROR: working tree not clean. Commit or stash first." >&2
  exit 1
fi

echo "==> Bumping version to $VERSION"

# tauri.conf.json
sed -i.bak -E "s/\"version\": \"[^\"]+\"/\"version\": \"$VERSION\"/" app/src-tauri/tauri.conf.json
rm -f app/src-tauri/tauri.conf.json.bak

# package.json
sed -i.bak -E "s/\"version\": \"[^\"]+\"/\"version\": \"$VERSION\"/" app/package.json
rm -f app/package.json.bak

# Cargo.toml (in src-tauri)
sed -i.bak -E "s/^version = \"[^\"]+\"/version = \"$VERSION\"/" app/src-tauri/Cargo.toml
rm -f app/src-tauri/Cargo.toml.bak
(cd app/src-tauri && cargo update -p solomd --offline --quiet 2>/dev/null || true)

git add app/src-tauri/tauri.conf.json app/package.json app/src-tauri/Cargo.toml app/src-tauri/Cargo.lock 2>/dev/null || true
git commit -m "chore: bump version to $VERSION"
git tag "v$VERSION"

echo ""
echo "==> Tagged v$VERSION"
echo "==> Push with: git push origin main && git push origin v$VERSION"
