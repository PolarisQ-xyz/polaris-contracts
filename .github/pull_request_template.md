## Summary

<!-- What does this PR do, and why? Reference the issue it addresses (fixes #N). -->

## Testing

<!-- How did you verify this? Which tests cover it, or what did you run manually against testnet? -->

## Funds-critical checklist

Complete this section if your change touches `contracts/*/src/lib.rs` (or any
code that moves balances, share accounting, or signature verification). See
the [funds-critical changes](./CONTRIBUTING.md#funds-critical-changes) section
of CONTRIBUTING.md for the reasoning behind each item.

- [ ] **Solvency asserted for multi-holder flows** — the invariant
  `collateral_token.balance(contract) == contract.total_supply` holds under
  every ordering of calls, and a test exercises the multi-holder path.
- [ ] **Stored struct shapes unchanged** — no fields added, removed, or
  reordered in `#[contracttype]` structs that are already persisted on-chain
  (or the migration is documented and tested).
- [ ] **WASM rebuilt and hash table updated** — `cargo build --release
  --target wasm32v1-none` was run and the recorded WASM hash table reflects
  the new build.
- [ ] **WASM copied into `polaris-oracle/wasm/`** — the rebuilt artifact was
  copied to the oracle-side `wasm/` directory so both sides stay in sync.
- [ ] **Oracle-side argument changes coordinated** — any change to the
  arguments the oracle passes (or expects) is reflected on the oracle side in
  the same PR or a linked one.
- [ ] **Regression test named after the bug** — a test that fails without the
  fix and passes with it, named for the bug it guards against (e.g.
  `test_cancelled_double_count`).

## General checklist

- [ ] `cargo test --workspace` passes.
- [ ] The PR is focused on one bug/feature/refactor.
- [ ] README/doc comments updated if documented behavior changed.
