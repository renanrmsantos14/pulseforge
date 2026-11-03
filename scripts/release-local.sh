#!/usr/bin/env bash
set -euo pipefail
project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$HOME/.cargo/env" 2>/dev/null || true
version_line="$(sed -n 's/^version = "\([0-9][0-9.]*\)"/\1/p' "$project_dir/Cargo.toml" | head -1)"
IFS=. read -r major minor patch <<< "$version_line"
next_version="$major.$minor.$((patch + 1))"
sed -i "0,/^version = \"$version_line\"/s//version = \"$next_version\"/" "$project_dir/Cargo.toml"
cargo build --release --features gui --manifest-path "$project_dir/Cargo.toml"
update_dir="${XDG_DATA_HOME:-$HOME/.local/share}/pulseforge/updates"
mkdir -p "$update_dir"
install -m 0755 "$project_dir/target/release/pulseforge" "$update_dir/pulseforge"
icon_dir="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"
mkdir -p "$icon_dir"
install -m 0644 "$project_dir/data/pulseforge.svg" "$icon_dir/pulseforge.svg"
printf 'PulseForge %s preparado em %s. Abra o app e clique em Check for updates.\n' "$next_version" "$update_dir/pulseforge"
