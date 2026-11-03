#!/usr/bin/env bash
# Publica a versão atual do Cargo.toml como release no GitHub (binário que o app baixa ao atualizar).
set -euo pipefail
project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$HOME/.cargo/env" 2>/dev/null || true
cd "$project_dir"
version="$(sed -n 's/^version = "\([0-9][0-9.]*\)"/\1/p' Cargo.toml | head -1)"
[ -z "$(git status --porcelain)" ] || { echo "Há alterações sem commit; faça commit antes de publicar." >&2; exit 1; }
cargo build --release --features gui
asset="$(mktemp -d)/pulseforge-linux-x86_64"
install -m 0755 target/release/pulseforge "$asset"
git push origin HEAD
gh release create "v$version" "$asset" --title "PulseForge $version" --generate-notes
echo "Release v$version publicada."
