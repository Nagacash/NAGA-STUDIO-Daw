"""Run after the native Mac build. Creates a Naga Studio DMG and optional publication ZIPs."""
from pathlib import Path
import hashlib, json, platform, re, shutil, subprocess, sys, tempfile, zipfile

APP_NAME = "Naga Studio"
EXE = "nagastudio"
PRODUCT_SLUG = "naga-studio"

if platform.system() != "Darwin":
    raise SystemExit("macOS is required.")
root = Path(__file__).resolve().parents[2]
app = Path(sys.argv[1]).resolve()
sys.path.insert(0, str(root / "packaging"))
from private_build import require_public_binary

require_public_binary(app / "Contents" / "MacOS" / EXE)
if any("Master Assistant" in str(p) for p in app.rglob("*")):
    raise SystemExit("Private assistant bundle cannot be published.")
version = re.search(r"pub const VERSION: u64 = (\d+);", (root / "crates/os-app/src/updates.rs").read_text()).group(1)
arch = platform.machine()
if arch not in ("arm64", "x86_64"):
    raise SystemExit("Unsupported architecture")
desktop = Path.home() / f"Desktop/{APP_NAME} Mac Public Download" / version
desktop.mkdir(parents=True, exist_ok=True)
repo_parent = root.parent
repo_parent.mkdir(parents=True, exist_ok=True)
source = desktop / f"Naga-Studio-Source-Mac-{arch}-{version}.zip"
subprocess.run([sys.executable, str(root / "packaging/export-source.py"), str(source), "--public"], check=True)
stage = Path(tempfile.mkdtemp(prefix="naga-mac-package-", dir=app.parent))
subprocess.run(["ditto", str(app), str(stage / f"{APP_NAME}.app")], check=True)
(stage / "Applications").symlink_to("/Applications")
for name in ("LICENSE", "THIRD-PARTY-NOTICES.txt"):
    shutil.copy2(root / name, stage / name)
shutil.copy2(source, stage / f"{APP_NAME} Source.zip")
(stage / "READ ME.txt").write_text(
    f"{APP_NAME}\n"
    f"Mac architecture: {arch}\n"
    "Drag the app to Applications (or ~/Applications). This build is ad-hoc signed, not notarized.\n"
    "Matching editable source and licence notices are included. Existing projects and settings are preserved.\n"
    "Built-in Naga plugins are installed to your user CLAP/VST3 folders when you run the release build script.\n"
)
dmg_name = f"Naga-Studio-{arch}-{version}.dmg"
dmg_desktop = desktop / dmg_name
dmg_repo = repo_parent / dmg_name
for dmg in (dmg_desktop, dmg_repo):
    if dmg.exists():
        dmg.unlink()
subprocess.run(["hdiutil", "create", "-volname", APP_NAME, "-srcfolder", str(stage), "-format", "UDZO", str(dmg_desktop)], check=True)
subprocess.run(["hdiutil", "verify", str(dmg_desktop)], check=True)
shutil.copy2(dmg_desktop, dmg_repo)
manifest = {
    "version": int(version),
    "label": f"Naga Studio Mac {arch}",
    "url": f"mac/{version}/{dmg_name}",
    "source": f"mac/{version}/{source.name}",
    "sha256": hashlib.sha256(dmg_desktop.read_bytes()).hexdigest(),
}
(desktop / f"mac-{arch}.json").write_text(json.dumps(manifest, indent=2))
with zipfile.ZipFile(desktop / f"Mac-{arch}-{version}-files.zip", "w", zipfile.ZIP_DEFLATED) as z:
    for p in (dmg_desktop, source):
        z.write(p, f"{PRODUCT_SLUG}/mac/{version}/{p.name}")
with zipfile.ZipFile(desktop / f"Mac-{arch}-{version}-publish-last.zip", "w", zipfile.ZIP_DEFLATED) as z:
    z.write(desktop / f"mac-{arch}.json", f"{PRODUCT_SLUG}/platforms/mac-{arch}.json")
print(f"DMG: {dmg_repo}\nAlso: {dmg_desktop}\nPublication zips: {desktop}")
