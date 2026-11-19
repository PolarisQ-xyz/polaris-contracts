# #92 — Optional wallet policy enforced over `auth_contexts`

Status: **design note only.** No policy is enforced on-chain today; this
document specifies one so the follow-up implementation has a target.

## 1. The gap

`contracts/smart-wallet`'s `__check_auth` verifies three things about the
WebAuthn assertion: the secp256r1 signature, the `challenge` binding to the
host-computed `signature_payload`, and the relying-party `type`/`origin`/
`rpIdHash` binding. All three answer *"did the owner's passkey approve this
payload?"* None answers *"what does this payload authorize?"* — the
`auth_contexts: Vec<Context>` argument is accepted and ignored.

Consequence: a single valid assertion authorizes **every** invocation in the
authorization tree the host built for that payload. The wallet's address can
be made to call any contract, any function, with any arguments, as long as
the wallet's address is permitted to make that call.

## 2. Threat model

- **Malicious or compromised frontend.** The page that drives
  `navigator.credentials.get()` decides what transaction it asks the host to
  simulate and sign. The browser's Face ID / Touch ID prompt shows no
  transaction details, so the owner approves what looks like an ordinary
  sign-in. The `origin` check only proves the ceremony happened on the
  pinned origin — it does not prove the *page* on that origin is honest
  (XSS, a compromised dependency, a malicious embedder iframe on the same
  registrable domain).
- **Malicious embedder / integrator.** Any dApp that can get the user to
  sign with this wallet can build an invocation tree of its choosing.
- **Out of scope.** A compromised authenticator or a stolen passkey (that is
  the key's problem, not the policy's); replay (already prevented by
  challenge binding); the factory's `deploy` path (unauthenticated by
  design, see README "Factory wasm pinning").

The asset at risk is everything the wallet address holds or can move:
market positions, vault shares, and native XLM.

## 3. Chosen policy

**A contract allowlist, with an optional native-XLM spend cap as a second,
independent rule.** Both are opt-in per wallet and both are enforced by
iterating `auth_contexts` *after* the existing signature/origin checks pass.

### 3.1 Contract allowlist (primary)

- Storage: `Vec<Address>` under a new instance key (e.g. `allowlist`),
  written by an owner-authorized `set_allowlist(Vec<Address>)` entrypoint.
  Empty/absent = policy disabled (current behaviour), so existing wallets
  are unaffected.
- Enforcement: for each `Context` in `auth_contexts`, require
  `context.contract` to be a member of the allowlist; otherwise return a new
  typed error (`Error::ContractNotAllowed`).
- Rationale for allowlist over a denylist: the wallet's legitimate call
  surface is small and known (market, perpetual, vault, the XLM token
  contract for sweeps). A denylist cannot enumerate the attacker's next
  contract.
- The allowlist must be able to include the native XLM SAC address, since
  generation sweeps (README "Migration procedure") are plain transfers.

### 3.2 Native XLM spend cap (secondary, optional)

- Storage: a per-transaction cap in stroops (`xlm_cap`), plus a rolling
  window total if a per-period cap is wanted later.
- Enforcement: for each `Context` whose `contract` is the native XLM SAC and
  whose function is `transfer`, decode the amount argument and require
  `amount <= cap`; otherwise `Error::SpendCapExceeded`.
- This is deliberately narrow: it only bounds *native* XLM, not SAC tokens
  or contract-internal balances. A general per-asset cap needs a token
  registry and is out of scope for the first implementation.
- Decoding `Context` arguments is the fragile part: `Context::Contract`
  carries `Args`, and the amount's position depends on the callee's
  signature. The implementation must fail closed — if the context is the
  XLM SAC but the arguments cannot be decoded as a `transfer(from, to,
  amount)`, reject rather than skip.

### 3.3 Why not "simulate and diff balances"

Not available inside `__check_auth`: the host calls it during authorization,
before the invocation runs, and the contract cannot simulate. Context
inspection is the only lever.

## 4. Enforcement point and ordering

Inside `__check_auth`, strictly after all existing checks (signature,
challenge, type, origin, `rpIdHash`, UP/UV flags) and before `touch(&env)`:

1. If no policy is configured, return `Ok(())` — unchanged behaviour.
2. Iterate `auth_contexts`; apply the allowlist rule to every context.
3. Apply the spend-cap rule to native-XLM `transfer` contexts.
4. Any violation returns a typed `Error` (never a panic), consistent with
   the repo's "errors, not panics" convention.

`Context::CreateContractHostFn` / `CreateContractWithCtorHostFn` variants
must be rejected outright when a policy is enabled — a wallet has no reason
to deploy contracts, and allowing it would let an attacker deploy a contract
the allowlist does not name and route value through it.

## 5. Upgrade interplay

- **Policy is per-wallet storage, not per-factory config.** It is set by the
  owner via an owner-authorized entrypoint, so it does not touch the
  factory's immutable `(wasm_hash, origin)` pair and does not require a new
  generation.
- **Existing wallets keep the old wasm.** A wallet deployed by an earlier
  factory generation runs the code pinned at its deploy time; it will not
  gain the policy until it is redeployed under a new generation. That is the
  same "redeploy rather than retrofit" pattern the README already documents
  for `origin` — and it means the policy is opt-in by construction for
  already-deployed wallets.
- **Sweeps must not be bricked.** A wallet with an allowlist that omits the
  XLM SAC cannot be swept to a new generation. The migration procedure must
  therefore require the owner to add the XLM SAC (and the destination
  contract, if any) before sweeping, or the policy setter must always
  implicitly allow the native XLM SAC. Recommendation: implicit allowance of
  the native XLM SAC, with the spend cap as the bound on it.
- **No admin override.** Consistent with the README's "no admin-gated
  in-place updates" decision, the factory admin must not be able to set or
  clear a wallet's policy. Only the wallet's own passkey can.

## 6. Follow-up implementation plan

Files, in order:

1. `contracts/smart-wallet/src/lib.rs`
   - New `Error` variants: `ContractNotAllowed`, `SpendCapExceeded`,
     `UnsupportedContext`, `PolicyArgsUndecodable`.
   - New storage keys `allowlist`, `xlm_cap`; `set_allowlist` /
     `set_xlm_cap` entrypoints guarded by `address.require_auth()` on the
     wallet's own address (which routes back through `__check_auth`).
   - Policy check helper called from `__check_auth` after the existing
     checks.
2. `contracts/smart-wallet/src/test.rs`
   - `valid_assertion_over_non_allowlisted_transfer_rejected`: init a wallet,
     set an allowlist that excludes the XLM SAC, present a genuinely signed
     assertion over a `transfer` context, assert `ContractNotAllowed`.
   - `valid_assertion_over_allowlisted_call_accepted`.
   - `spend_cap_rejects_over_cap_transfer` and the at-cap boundary.
   - `policy_disabled_by_default_accepts_any_context` (regression guard for
     existing wallets).
3. `README.md` — replace the "design note only" wording in
   [Wallet policy](../..#wallet-policy-what-a-passkey-assertion-authorizes)
   with the enforced behaviour.

Done when: the four tests above pass, `cargo test --workspace` is green, and
a wallet with no policy configured behaves byte-for-byte as today.
