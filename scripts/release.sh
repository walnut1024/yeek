#!/usr/bin/env bash
set -euo pipefail

# ── Config ──────────────────────────────────────────────────────────
ENV_FILE="$(cd "$(dirname "$0")/.." && pwd)/.release.env"
if [[ ! -f "$ENV_FILE" ]]; then
  echo "Error: $ENV_FILE not found. Copy .release.env.example and fill in values."
  exit 1
fi
source "$ENV_FILE"

if [[ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]]; then
  echo "Error: TAURI_SIGNING_PRIVATE_KEY_PASSWORD not set in $ENV_FILE"
  exit 1
fi

for var in APPLE_SIGNING_IDENTITY APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_PATH; do
  if [[ -n "${!var:-}" ]]; then
    export "$var"
  fi
done

# ── Args ────────────────────────────────────────────────────────────
VERSION="${1:?Usage: scripts/release.sh <version> [release-notes]}"
NOTES="${2:-Release v$VERSION}"

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"
BUNDLE_DIR="target/release/bundle"
APP_PATH="$BUNDLE_DIR/macos/Yeek.app"
BUILD_CONFIG=""

cleanup() {
  [[ -n "$BUILD_CONFIG" && -f "$BUILD_CONFIG" ]] && rm -f "$BUILD_CONFIG"
}
trap cleanup EXIT

# ── Validate version format ─────────────────────────────────────────
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+ ]]; then
  echo "Error: Version must be semver (e.g. 2.0.0-alpha.5), got: $VERSION"
  exit 1
fi

# ── Guard: clean working tree ───────────────────────────────────────
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Error: Working tree has uncommitted changes. Commit or stash first."
  git status --short
  exit 1
fi

# ── Bump version ────────────────────────────────────────────────────
echo "→ Bumping version to $VERSION in config files..."
sed -i '' "s/\"version\": \"[^\"]*\"/\"version\": \"$VERSION\"/" \
  src-tauri/tauri.conf.json package.json
sed -i '' "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" \
  src-tauri/Cargo.toml
python3 - "$VERSION" <<'PY'
import re
import sys
from pathlib import Path

version = sys.argv[1]
path = Path("Cargo.lock")
content = path.read_text()
pattern = re.compile(r'(\[\[package\]\]\nname = "yeek"\nversion = ")[^"]+(")')
content, count = pattern.subn(rf"\g<1>{version}\2", content, count=1)
if count != 1:
    raise SystemExit("failed to update yeek version in Cargo.lock")
path.write_text(content)
PY

# ── Commit & tag ────────────────────────────────────────────────────
echo "→ Committing version bump..."
git add src-tauri/tauri.conf.json package.json src-tauri/Cargo.toml Cargo.lock
git commit -m "release: v$VERSION"
git tag "v$VERSION"

# ── Push ────────────────────────────────────────────────────────────
echo "→ Pushing to remote..."
git push
git push --tags

# ── Build ───────────────────────────────────────────────────────────
echo "→ Building signed release..."
export TAURI_SIGNING_PRIVATE_KEY
TAURI_SIGNING_PRIVATE_KEY="$(cat "$TAURI_SIGNING_PRIVATE_KEY_PATH")"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD

BUILD_ARGS=(cargo tauri build)
if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  echo "→ Using Apple signing identity: $APPLE_SIGNING_IDENTITY"
  BUILD_CONFIG="$(mktemp)"
  python3 - "$BUILD_CONFIG" "$APPLE_SIGNING_IDENTITY" <<'PY'
import json
import sys

path, identity = sys.argv[1], sys.argv[2]
with open(path, "w", encoding="utf-8") as f:
    json.dump({"bundle": {"macOS": {"signingIdentity": identity}}}, f)
PY
  BUILD_ARGS+=(--config "$BUILD_CONFIG")
else
  echo "Warning: APPLE_SIGNING_IDENTITY is not set; using ad-hoc macOS signing."
fi

if [[ -n "${APPLE_ID:-}" && -n "${APPLE_PASSWORD:-}" && -n "${APPLE_TEAM_ID:-}" ]] || \
   [[ -n "${APPLE_API_KEY:-}" && -n "${APPLE_API_ISSUER:-}" && -n "${APPLE_API_KEY_PATH:-}" ]]; then
  echo "→ Apple notarization credentials detected."
else
  echo "Warning: Apple notarization credentials are not set; downloaded apps will still show a Gatekeeper warning."
fi

"${BUILD_ARGS[@]}"

# ── Verify macOS bundle ─────────────────────────────────────────────
echo "→ Verifying macOS app signature..."
codesign --verify --deep --strict --verbose=4 "$APP_PATH"
if ! spctl --assess --type execute --verbose=4 "$APP_PATH"; then
  echo "Warning: Gatekeeper assessment rejected the app."
  echo "Warning: Configure an Apple Developer ID certificate and notarization for fully trusted public distribution."
fi

# ── Generate latest.json ───────────────────────────────────────────
SIG="$(cat "$BUNDLE_DIR/macos/Yeek.app.tar.gz.sig")"
PUB_DATE="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

cat > "$BUNDLE_DIR/macos/latest.json" <<EOF
{
  "version": "$VERSION",
  "notes": $(echo "$NOTES" | python3 -c 'import sys,json; print(json.dumps(sys.stdin.read()))'),
  "pub_date": "$PUB_DATE",
  "platforms": {
    "darwin-aarch64": {
      "signature": "$SIG",
      "url": "https://github.com/walnut1024/yeek/releases/download/v$VERSION/Yeek.app.tar.gz"
    }
  }
}
EOF

echo "→ latest.json generated"

# ── GitHub Release ──────────────────────────────────────────────────
DMG="$(ls "$BUNDLE_DIR/dmg/Yeek_${VERSION}"_*.dmg 2>/dev/null | head -1)"
RELEASE_ARGS=(
  "$BUNDLE_DIR/macos/Yeek.app.tar.gz"
  "$BUNDLE_DIR/macos/Yeek.app.tar.gz.sig"
  "$BUNDLE_DIR/macos/latest.json"
)
[[ -n "$DMG" ]] && RELEASE_ARGS+=("$DMG")

echo "→ Creating GitHub Release v$VERSION..."
gh release create "v$VERSION" \
  "${RELEASE_ARGS[@]}" \
  --title "v$VERSION" \
  --notes "$NOTES"

echo "✓ Released v$VERSION"
gh release view "v$VERSION" --web
