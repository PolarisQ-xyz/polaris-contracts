# Redeploy vs. In-Place Upgrade Decision Criteria

Because Polaris comprises both long-lived singleton contracts (`polaris-vault`, `polaris-smart-wallet-factory`), per-user deterministic instances (`polaris-smart-wallet`), and per-market instances (`polaris-market`, `polaris-perpetual`), fixing a bug or changing configuration requires choosing between **deploying a fresh contract instance ("redeploy rather than retrofit")** and **upgrading code in place**.

This document consolidates the decision criteria referenced across the repository (including the vault bug-16 redeploy, the smart-wallet-factory WebAuthn origin-binding redeploy, and the emergency-pause / admin-rotation additions).

---

## 1. Current On-Chain Upgrade Posture

None of the contracts in this workspace currently expose `env.deployer().update_current_contract_wasm(...)`. Every deployed contract instance executes the exact Wasm bytecode it was deployed with for its entire lifetime.

Even if an in-place Wasm upgrade entrypoint were added in the future, **a fresh contract deployment is mandatory** whenever any of the criteria in Section 2 apply.

---

## 2. When a Fresh Redeploy Is Mandatory ("Redeploy Rather Than Retrofit")

Deploy a **new contract ID** (and retire/drain the old instance) whenever any of the following hold:

1. **No In-Place Wasm Update Entrypoint Exists on the Live Instance:**
   - Because deployed instances do not implement `update_current_contract_wasm`, any bytecode change (new entrypoints such as `pause`/`unpause`, bug fixes in settlement or LP share accounting, or ABI changes) requires deploying a new contract instance.
2. **Corrupted or Untrusted Instance Storage / Broken Invariant (e.g., Vault Bug 16):**
   - If a historical bug allowed instance storage (`total_supply`, share ledgers, or admin address) to drift from the true collateral balance, replacing the Wasm code in place does not repair the corrupted ledger state. Abandoning the old instance and deploying a clean one guarantees the `collateral.balance(contract) == total_supply` invariant holds from block zero.
3. **Immutable Generation Parameters on `polaris-smart-wallet-factory` (e.g., Origin Binding):**
   - A factory's `(wasm_hash, origin)` pair is set once in `__constructor` and has no setter. Because wallet addresses are derived deterministically from `(factory_address, sha256(public_key))`, changing the wallet Wasm (such as adding WebAuthn `origin`/`type` enforcement) or changing the relying-party origin **must** create a new factory generation (`G2`). Existing `G1` wallets continue executing their original Wasm undisturbed until users sweep funds to their `G2` address.
4. **Compromised or Lost Admin Key:**
   - If the current `admin` secret key is lost or compromised before `set_admin` can be called, the instance cannot be safely administered and must be superseded by a freshly deployed instance.
5. **Breaking Storage Layout or Error ABI Changes:**
   - Renaming `#[contracttype]` struct fields or changing storage key encodings in a way that cannot deserialize existing instance/persistent entries requires a fresh instance.

---

## 3. When Existing Instances Can Be Kept (or Upgraded In-Place)

A fresh redeploy of existing instances is **not** required when:

1. **Per-Market Rollout (`polaris-market`):**
   - Dated markets are short-lived and each market is its own contract instance deployed on demand by `polaris-oracle`. Updating `MARKET_WASM_HASH` in `polaris-oracle` immediately applies the new Wasm to all newly created markets while existing open markets simply run to their natural `expiry` (or `expiry + grace_period`) and settle/redeem under their original bytecode.
2. **Additive, Backward-Compatible Storage Keys (When Combined With Future In-Place Wasm Upgrades):**
   - Features like `DataKey::Paused` / `DataKey::DepositsPaused` default to `false` when the storage key is absent, requiring zero storage migration.
3. **Off-Chain Tooling, Interface Docs, or Test-Only Changes:**
   - Changes confined to `polaris-oracle-config`, tests, or documentation that do not alter compiled contract Wasm.

---

## 4. Per-Contract Redeploy Checklist

| Contract | How to Roll Out a Wasm Change | How Existing Funds Exit the Old Instance |
|---|---|---|
| `polaris-market` | Upload new Wasm; update `MARKET_WASM_HASH` in `polaris-oracle` for new markets. | Existing markets resolve at `expiry` (or `cancel` after `grace_period`) and holders call `redeem`. |
| `polaris-perpetual` | Upload new Wasm; deploy a new perpetual instance and update `polaris-oracle`. | Admin calls `terminate` on the old perpetual; holders and treasury call `redeem`. |
| `polaris-vault` | Deploy a new vault via `__constructor(admin, collateral)`; update backend config. | Admin calls `pause_deposits` on the old vault, redeems settled market positions via `redeem_from_market`, and LPs call `withdraw`. |
| `polaris-smart-wallet-factory` | Upload new wallet Wasm; deploy a new factory generation (`G_n+1`) via `__constructor`. | Stop routing new sign-ups to `G_n`; prompt users to sign a passkey sweep from their `G_n` wallet address to `G_n+1`. |
