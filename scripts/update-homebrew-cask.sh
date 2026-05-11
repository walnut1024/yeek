#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:?Usage: scripts/update-homebrew-cask.sh <version> <dmg-path>}"
DMG_PATH="${2:?Usage: scripts/update-homebrew-cask.sh <version> <dmg-path>}"

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

if [[ ! -f "$DMG_PATH" ]]; then
  echo "Error: DMG not found: $DMG_PATH"
  exit 1
fi

TAP_REPO="${HOMEBREW_TAP_REPO:-walnut1024/homebrew-yeek}"
TAP_REMOTE="https://github.com/${TAP_REPO}.git"
SHA256="$(shasum -a 256 "$DMG_PATH" | awk '{print $1}')"
TMP_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

echo "→ Updating Homebrew tap ${TAP_REPO} for v${VERSION}..."
git clone "$TAP_REMOTE" "$TMP_DIR/tap"
cd "$TMP_DIR/tap"
mkdir -p Casks

python3 - "$VERSION" "$SHA256" > Casks/yeek.rb <<'PY'
import sys

version, sha256 = sys.argv[1], sys.argv[2]
print(f'''cask "yeek" do
  version "{version}"
  sha256 "{sha256}"

  url "https://github.com/walnut1024/yeek/releases/download/v#{{version}}/Yeek_#{{version}}_aarch64.dmg"
  name "Yeek"
  desc "Session memory manager for local agent sessions"
  homepage "https://github.com/walnut1024/yeek"

  livecheck do
    url :url
    strategy :github_latest
  end

  auto_updates true
  depends_on arch: :arm64

  app "Yeek.app"

  zap trash: [
    "~/Library/Application Support/dev.yeek.app",
    "~/Library/Caches/dev.yeek.app",
    "~/Library/Logs/dev.yeek.app",
    "~/Library/Preferences/dev.yeek.app.plist",
  ]
end''')
PY

git add Casks/yeek.rb
if git diff --cached --quiet; then
  echo "→ Homebrew cask already up to date."
  exit 0
fi

git commit -m "Update Yeek to v${VERSION}"
git push origin HEAD
echo "✓ Homebrew tap updated: ${TAP_REPO}"
