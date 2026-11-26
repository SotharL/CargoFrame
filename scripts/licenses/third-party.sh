#!/usr/bin/env bash
# THIRD-PARTY-LICENSES.md: the license of every crate CargoFrame ships with (cargo-about, from the
# crates' own sources), then the two JavaScript packages built into the windows. Run after a
# dependency changes, and before a release; the installers carry the file.
#   cargo install cargo-about --locked --features cli
#   scripts/licenses/third-party.sh
set -euo pipefail
cd "$(dirname "$0")/../.."
out=THIRD-PARTY-LICENSES.md
cargo about generate --fail scripts/licenses/about.hbs -o "$out"
for pkg in svelte @tauri-apps/api; do
  dir="ui/node_modules/$pkg"
  version=$(node -p "require('./$dir/package.json').version")
  license=$(node -p "require('./$dir/package.json').license")
  file=$(ls "$dir"/LICENSE* 2>/dev/null | head -n1)
  {
    printf '\n## %s %s (%s), in the windows\n\n```\n' "$pkg" "$version" "$license"
    cat "$file"
    printf '\n```\n'
  } >> "$out"
done
echo "wrote $out ($(wc -c < "$out") bytes)"
