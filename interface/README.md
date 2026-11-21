# polaris-interface

Typed Soroban clients and contract types for the Polaris contracts:
`polaris-market`, `polaris-perpetual`, `polaris-vault`,
`polaris-smart-wallet` and `polaris-smart-wallet-factory`. Use it as a
normal Cargo dependency instead of copying a `.wasm` file into your own
`contractimport!`.

```toml
[dependencies]
polaris-interface = "0.1"

[dev-dependencies]
polaris-interface = { version = "0.1", features = ["testutils"] }
```

```rust
use polaris_interface::{market, vault};

let market = market::Client::new(&env, &market_id);
let (yes_bps, no_bps) = market.get_price();

// In tests: deploy a real market from the released bytes.
let id = env.register(market::WASM, ());
```

Each module is one contract's `contractimport!` output: `Client` (with
`try_*` variants), every type the interface uses, the `Error` enum, and
`WASM`. The crate is `rlib`-only and exports no `#[contract]`, so
depending on it from your own contract can't cause a wasm export-symbol
collision.

## Versioning

The crate version is the contract release whose wasm is vendored in
`wasm/`. `wasm/SHA256SUMS` lists each file's hash, which is also the
on-chain wasm hash of any instance deployed from that release, so you can
check that a contract you're calling runs the code this crate describes.

## How it's built, and what CI checks

The convention matches other published Soroban SDK crates, e.g. Blend's
`blend-contract-sdk`: vendor the release wasm, wrap it in
`contractimport!`.

- `scripts/sync-interface-wasm.sh` rebuilds every release wasm and copies
  it into `wasm/` with a fresh `SHA256SUMS`.
- `vendored_specs_match_current_source` compares the interface spec
  (functions, types, errors, docs) embedded in each vendored wasm with a
  fresh build of the current source. Any interface change without a
  re-sync fails CI. It compares specs, not bytes, because a toolchain
  difference can change the code bytes without changing the interface.
- `vendored_wasm_matches_sha256sums` fails if a vendored file no longer
  matches its listed hash.

## Releasing

1. Bump `version` in `interface/Cargo.toml` to the contract release.
2. Run `scripts/sync-interface-wasm.sh` and commit `interface/wasm/`.
3. `cargo test --workspace`, then `cargo publish -p polaris-interface`.

TypeScript bindings can be generated from the same files:
`stellar contract bindings typescript --wasm interface/wasm/polaris_market.wasm --output-dir <dir>`.
