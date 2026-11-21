#!/usr/bin/env bash
# Rebuilds the release wasm for every contract polaris-interface exposes and
# vendors it into interface/wasm/, with a SHA256SUMS manifest — the release
# step for polaris-interface (see interface/README.md). Run from anywhere.
#
# polaris-market is built on its own first, for the same reason CI does it:
# polaris-vault's contractimport! reads market's compiled wasm as a file at
# macro-expansion time, a dependency Cargo's own build graph can't see.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

contracts=(polaris_market polaris_perpetual polaris_vault polaris_smart_wallet polaris_smart_wallet_factory)

cargo build --release --target wasm32v1-none -p polaris-market
cargo build --release --target wasm32v1-none \
  -p polaris-perpetual -p polaris-vault -p polaris-smart-wallet -p polaris-smart-wallet-factory

out="interface/wasm"
mkdir -p "$out"
for c in "${contracts[@]}"; do
  cp "target/wasm32v1-none/release/$c.wasm" "$out/$c.wasm"
done
(cd "$out" && shasum -a 256 "${contracts[@]/%/.wasm}" > SHA256SUMS)

echo "Vendored into $out:"
cat "$out/SHA256SUMS"
