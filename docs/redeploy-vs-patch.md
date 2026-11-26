# Redeploy vs. patch: which one, and why

Several incidents in this repo were resolved by **abandoning a deployed
instance and deploying a fresh one** instead of retrofitting the fix. This
doc records the actual decision criteria so the next person doesn't have to
re-derive them from three separate README stories.

## Soroban's upgrade model in one paragraph

Soroban stores a contract's wasm hash separately from its instance address.
A contract can be updated in place *only* if its deployed wasm already exposes
an entrypoint that calls `update_current_contract_wasm`, guarded by whatever
authorization that code specifies. That capability cannot be added later from
outside: if the original wasm has no upgrade hook, there is no way to make an
existing instance run new code. Updating the wasm also only swaps code — it
does not repair already-corrupted or stranded state, and storage keys written
by the old wasm must still mean the same thing to the new one. Finally, some
config in this repo is deliberately pinned at `initialize()` with no setter,
so even a working upgrade hook cannot change it.

## Decision criteria

**Redeploy a fresh instance** when any of these holds:

1. **No upgrade hook in the deployed wasm.** All contracts in this repo ship
   without one, so today this is the default answer for *any* code fix to a
   live instance. Adding a hook now only helps instances deployed with it.
2. **State is stranded or corrupt.** If funds are already locked behind a
   missing setter (the vault's original admin-rotation incident) or a bad
   storage value, new code cannot reach the assets — moving capital to a
   correctly initialized instance is the recovery.
3. **An init-pinned value must change.** The factory's `wasm_hash` and
   relying-party `origin`, and each market's `reflector_decimals_at_init`, are
   immutable after `initialize()`. Changing one means a new generation, not a
   patch.
4. **Old instances must keep their old behavior.** For the wallet factory,
   deployed wallets keep their original wasm; a new factory is a new
   generation and does not upgrade or invalidate existing wallets. Keeping
   both generations live is intentional, not a migration failure.
5. **The storage shape or semantics changed.** If a fix reinterprets an
   existing key or changes a `#[contracttype]` layout, old instances can't be
   safely upgraded into the new meaning.

**Patch in place** (upgrade the wasm at the same address) only when *all* of
these hold:

- the deployed wasm already had an upgrade hook with working authorization;
- no state needs repairing and the storage layout is unchanged;
- nothing pinned at initialization needs to change; and
- no holder needs to remain on the old behavior — i.e. one address's code
  changing under users is acceptable.

Write down which branch you took and why in the PR that ships the fix, and
cross-link it here.

## Precedents

| Incident | Decision | Why |
|---|---|---|
| Vault admin rotation / bug 16 (`polaris-oracle`) | Fresh instance | Original instance had no `accept_admin`; funds were unreachable, and no upgrade hook existed. See README's "Emergency pause" / vault section. |
| Smart-wallet factory origin binding | Fresh factory generation | `origin` and `wasm_hash` are init-pinned, and existing wallets must keep running their original wasm. See README's "Factory generations and migration". |
| Vault pause / admin-rotation follow-ups | **Patch in code, redeploy still pending** | The fixes (`propose_admin`/`accept_admin`, pause surviving rotation) are in this repo, but live instances predate them and have no upgrade hook — so they need fresh instances before the fix applies on-chain. |

Until a contract ships with a deliberate, authorized upgrade hook, treat
"fixed in the repo" and "fixed on the deployed instance" as two different
states.

## Pinned dependency: `soroban-sdk = "26.1.0"`

The workspace `Cargo.toml` pins `soroban-sdk = "26.1.0"`. The latest published
release is `30.0.0-rc.1`, so the pin is four majors behind. This is recorded
here as a **deliberate decision to track**, not as neglect and not as a
proposal to bump now.

Unlike a typical crate, soroban-sdk majors generally track specific Stellar
protocol versions. Jumping to a release candidate, or skipping several majors,
is not a routine bump and could easily be wrong to do reflexively — the SDK
version must match the protocol the target network actually runs.

### Open questions (to confirm, not assume)

- Which Stellar protocol version does `26.1.0` correspond to?
- Which protocol does each later major (27, 28, 29, 30) map to?
- Has the testnet/mainnet this project targets moved past what `26.1.0`
  supports?

If staying on `26.1.0` already matches the target network's protocol, it is
the correct choice and should stay pinned. If the target network has moved
past it, the gap has grown unintentionally and a coordinated bump (SDK +
contracts + redeploy) is warranted. Either way, record the outcome here so the
next reader knows the pin was checked, not forgotten.
