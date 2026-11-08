#!/bin/bash
# Build Naga Studio for macOS: app, all bundled plugins, DMG at repo root.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
[[ "$(uname -s)" == Darwin ]] || { echo "Run on macOS."; exit 1; }
[[ "$(sysctl -in sysctl.proc_translated 2>/dev/null || true)" != 1 ]] || { echo "Use a native arm64 terminal, not Rosetta."; exit 1; }
export PATH="$HOME/.cargo/bin:$PATH"
# Keep artifacts on the repo volume (e.g. external SSD); ~/.cache fills the boot drive quickly.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
need_kib=2048000 # ~2 GiB headroom for release + all plugins
avail_kib="$(df -k "$CARGO_TARGET_DIR" 2>/dev/null | awk 'NR==2 {print $4}')"
if [[ -n "$avail_kib" && "$avail_kib" -lt "$need_kib" ]]; then
  echo "Need at least 2 GiB free on $(df -h "$CARGO_TARGET_DIR" | awk 'NR==2 {print $1}') (have $(df -h "$CARGO_TARGET_DIR" | awk 'NR==2 {print $4}')). Free space or set CARGO_TARGET_DIR." >&2
  exit 1
fi
xcode-select -p >/dev/null
cd "$ROOT"
echo "== Building Naga Studio (release) =="
cargo build --locked --release -p os-app
echo "== Installing Naga CLAP/VST3 plugins =="
cargo run --locked --release -p bundle-plugins -- --install
echo "== Bundling Naga Studio.app =="
if [[ "${NAGA_SKIP_INSTALL:-0}" == 1 ]]; then
  cargo run --locked --release -p bundle-app
else
  cargo run --locked --release -p bundle-app -- --install
fi
APP="$CARGO_TARGET_DIR/release/bundled/Naga Studio.app"
[[ -d "$APP" ]] || APP="$CARGO_TARGET_DIR/bundled/Naga Studio.app"
[[ -d "$APP" ]] || { echo "Bundle missing at $CARGO_TARGET_DIR"; exit 1; }
codesign --verify --deep --strict "$APP"
python3 "$ROOT/packaging/mac/package-release.py" "$APP"
if [[ "${NAGA_SKIP_INSTALL:-0}" == 1 ]]; then
  echo "Done. DMG is on Desktop and in the NagaDAW folder. Install with: cargo run --release -p bundle-app -- --install"
else
  echo "Done. Naga Studio.app is in ~/Applications (replaced each build). DMG is also on Desktop and in the NagaDAW folder."
  echo "DMG-only install: NAGA_SKIP_INSTALL=1 bash packaging/mac/build-naga-dmg.command"
fi
