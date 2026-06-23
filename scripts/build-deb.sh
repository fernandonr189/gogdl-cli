#!/usr/bin/env bash
# Builds gogdl in release mode and packages it as a .deb for Ubuntu/Debian.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

if ! command -v cargo-deb >/dev/null 2>&1; then
    echo "cargo-deb not found, installing it via 'cargo install cargo-deb'..."
    cargo install cargo-deb
fi

cargo deb

deb_path=$(find target/debian -maxdepth 1 -name '*.deb' -printf '%T@ %p\n' | sort -n | tail -1 | cut -d' ' -f2-)
echo "Built package: $deb_path"
echo "Install it with: sudo apt install ./$deb_path"
