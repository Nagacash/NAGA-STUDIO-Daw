# Naga Studio DAW

Mac community preview • 0.0.1

Naga Studio is a music production digital audio workstation for macOS, built on native Apple-silicon (arm64) and Intel (x86_64) executables with a macOS 11.0 minimum target.

## Install

Download `Naga-Studio-Installer.dmg`, open it, and run `Naga-Studio-0.0.1-macOS-Universal-UNSIGNED.pkg`.

This package is **UNSIGNED and NOT NOTARIZED** by Apple. The app is locally (ad-hoc) signed. macOS may block the download — approve it through System Settings > Privacy & Security. Do not disable Gatekeeper globally.

## Contents

- `Naga-Studio-Installer.dmg` — disk image installer
- `Naga Studio Source.zip` — matching modified source (GPL-3.0-or-later)
- `LICENSE` — GPL-3.0-or-later
- `THIRD-PARTY-NOTICES.txt` — third-party notices

## Plugins

Install and license your own compatible VST3, VST2 or CLAP plugins. Audio Units, AAX and FL Studio-exclusive plugins are not supported directly. An Apple-silicon process cannot load Intel-only plugins.

## Remove

Quit Naga Studio and remove `/Applications/Naga Studio.app`. Your projects and `~/Library/Application Support/Naga Studio` remain untouched.
