#!/bin/bash
set -euo pipefail
project_dir="$(cd "$(dirname "$0")" && pwd)"
plugin_source="$project_dir/build/RebitRust_artefacts/Release/AU/Rebit Rust.component"
plugin_destination="$HOME/Library/Audio/Plug-Ins/Components"
if [ ! -d "$plugin_source" ]; then
    printf 'Build not found. Run scripts/build.sh first.\n'
    exit 1
fi
mkdir -p "$plugin_destination" "$project_dir/dist/au-backups"
stage_root="$(mktemp -d "$plugin_destination/.rebit-rust-install.XXXXXX")"
trap 'rm -rf "$stage_root"' EXIT
staged="$stage_root/Rebit Rust.component"
installed="$plugin_destination/Rebit Rust.component"
backup="$project_dir/dist/au-backups/Rebit Rust-$(date +%Y%m%d-%H%M%S)-$$.component"
ditto "$plugin_source" "$staged"
codesign --force --deep --sign - "$staged"
codesign --verify --deep --strict "$staged"
if [ -e "$installed" ]; then mv "$installed" "$backup"; fi
if ! mv "$staged" "$installed"; then
    if [ -e "$backup" ]; then mv "$backup" "$installed"; fi
    exit 1
fi
printf '\nInstalled Rebit Rust.component.\nRestart Logic, then select AU Instruments > Rebit Audio > Rebit Rust.\nThe original Rebit is unchanged.\n'
