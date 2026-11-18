# Threat Model & Trust Assumptions

This document is the single source of truth for the trust assumptions of the
contracts in this repository. It lists every actor, what they can do per
contract, the worst case if they are malicious or compromised, the mitigations
that exist today, and the risks we knowingly accept.

Auditors (#16) and the mainnet checklist (#15) should use this document to
verify each role's reach. If a change alters who can do what, update this file
in the same PR.

## Scope

Contracts covered:

| Contract | Path |
| --- | --- |
| `ctf-math` | `contracts/ctf-math` |
| `market` | `contracts/market` |
| `perpetual` | `contracts/perpetual` |
| `vault` | `contracts/vault` |
| `smart-wallet` | `contracts/smart-wallet` |
| `smart-wallet-factory` | `contracts/smart-wallet-factory` |
| mock contracts (`mock-lazer`, `mock-redstone`) | `contracts/mock-lazer`, `contracts/mock-redstone` |

Oracle providers covered: **Pyth / Lazer**, **Reflector**, and **RedStone**.

## Actors

| Actor | Trust level |
| --- | --- |
| Market admin | Trusted for configuration, not for funds |
| Perpetual admin | Trusted for lifecycle, not for funds |
| Vault admin | **Fully trusted** — controls all pooled funds |
| Factory admin | Trusted only at initialization |
| Any user | Untrusted; can call permissionless entry points |
| Pyth / Lazer operator | Trusted to be honest and not collude |
| Reflector operator | Trusted to be honest and not collude |
| RedStone operator | Trusted to be honest and not collude |

## Capabilities per contract

### `ctf-math`

Pure math library. No storage, no admin, no external calls.

- **Actors:** none.
- **Worst case:** none — correctness is enforced by tests and by callers.
- **Mitigations:** unit tests; callers validate inputs.
- **Accepted risks:** none.

### `market`

- **Market admin** can:
  - pause / unpause the market,
  - set the oracle configuration, including `lazer_contract` and other
    oracle addresses,
  - update market parameters within configured bounds.
- **Any user** can call `settle`, `cancel`, and `settle_via_reflector`.
- **Worst case (admin malicious/compromised):** the admin can point the market
  at an attacker-controlled `lazer_contract`, which effectively lets the admin
  choose prices, and can pause trading to grief users. The admin cannot move
  user funds directly.
- **Worst case (permissionless caller malicious):** `settle`, `cancel`, and
  `settle_via_reflector` are permissionless by design; a caller can only trigger
  settlement/cancellation that the contract logic already permits. They cannot
  fabricate outcomes beyond what the configured oracle reports.
- **Mitigations:** oracle address changes are admin-gated; settlement reads from
  the configured oracle; timestamp windows and tolerance bounds constrain when
  and how a price is accepted.
- **Accepted risks:** the admin is trusted not to point `lazer_contract` at a
  malicious contract (see the mock-lazer network guard issue). Timestamp-window
  and tolerance-bound gaps found during audit are tracked as follow-up issues.

### `perpetual`

- **Perpetual admin** can terminate the perpetual.
- **Any user** can interact with permissionless entry points (open/close,
  settle, cancel) subject to contract checks.
- **Worst case (admin malicious/compromised):** the admin can terminate the
  perpetual, halting new activity and forcing wind-down. The admin cannot move
  user funds directly.
- **Mitigations:** termination is a bounded lifecycle action; existing positions
  follow the documented wind-down path.
- **Accepted risks:** the admin is trusted to terminate only for legitimate
  reasons (e.g. oracle failure).

### `vault`

- **Vault admin** controls all pooled funds: it can move, allocate, and withdraw
  pooled assets.
- **Worst case (admin malicious/compromised):** total loss of pooled funds. This
  is the highest-trust role in the system.
- **Mitigations:** none at the contract level beyond access control — the admin
  is fully trusted by design.
- **Accepted risks:** vault depositors trust the vault admin. This must be
  called out in user-facing docs and in the mainnet checklist (#15).

### `smart-wallet`

- **Owner** controls the wallet's own funds and can authorize calls.
- **Worst case (owner compromised):** loss of that wallet's funds only; no
  cross-wallet impact.
- **Mitigations:** per-wallet ownership; no shared state with other wallets.
- **Accepted risks:** standard smart-wallet key-management risk borne by the
  owner.

### `smart-wallet-factory`

- **Factory admin** is trusted **only at initialization** (e.g. to set the
  initial configuration / implementation).
- **Worst case (admin malicious/compromised):** if compromised before
  initialization completes, the admin could set a malicious initial
  configuration. After initialization the admin has no ongoing power.
- **Mitigations:** initialization is a one-time action; post-init the admin
  cannot reconfigure existing wallets.
- **Accepted risks:** trust in the factory admin is limited to the init window.

### Mock contracts (`mock-lazer`, `mock-redstone`)

- Test-only contracts. Not deployed to production.
- **Worst case:** none in production; in tests they return whatever the test
  configures.
- **Mitigations:** excluded from production deployments; the mock-lazer network
  guard issue tracks preventing accidental production use.
- **Accepted risks:** none, provided they are never deployed to mainnet.

## Oracle providers

All three oracle operators are **trusted to be honest and not to collude**.
A malicious or colluding oracle can report arbitrary prices, which flows into
`market` and `perpetual` settlement.

| Provider | Used by | Worst case if malicious/compromised | Mitigations | Accepted risks |
| --- | --- | --- | --- | --- |
| Pyth / Lazer | `market`, `perpetual` | Arbitrary prices → incorrect settlement and payouts | Timestamp windows and tolerance bounds; admin-selected `lazer_contract` | Operator honesty assumed; admin must select a legitimate contract |
| Reflector | `market` (`settle_via_reflector`) | Arbitrary prices → incorrect settlement | Same bounds as above | Operator honesty assumed |
| RedStone | `market`, `perpetual` | Arbitrary prices → incorrect settlement | Same bounds as above | Operator honesty assumed |

## Accepted risks summary

- Vault admin is fully trusted with pooled funds.
- Market admin can effectively choose prices by selecting `lazer_contract`.
- Perpetual admin can terminate at will.
- Factory admin is trusted only during initialization.
- Pyth/Lazer, Reflector, and RedStone operators are trusted to be honest and
  not to collude.
- Timestamp-window and tolerance-bound gaps found during audit are tracked as
  follow-up issues.

## Related issues

- #16 — independent audit (depends on this document)
- #15 — mainnet checklist (depends on this document)
- Mock-lazer network guard issue — prevents production use of mock oracles
