# Storage layout

This document is the single source of truth for every Soroban storage key used
by Polaris contracts: which storage class it lives in (instance vs.
phemeral vs. persistent), its value type, and its TTL bump policy. It also
records the rules that must be followed when changing storage.

If you add, remove, or change a key, update this file in the same PR. See the
"Changing storage" checklist in `CONTRIBUTING.md`.

## Storage classes and TTL policy

Soroban has three storage classes:

- **Instance** — lives alongside the contract instance. Bumped together with
the instance entry. Use for small, frequently-read config that must always be
available.
- **Persistent** — survives archival; must be explicitly bumped or it can be
archived and require a restore. Use for balances and long-lived per-user data.
- **Temporary** — expires and is never restored. Not used by Polaris today.

All Polaris contracts use the same bump constants (defined per crate):

| Constant         | Value     | Meaning                          |
| ---------------- | --------- | -------------------------------- |
| `BUMP_TO`        | `535_680` | Bump to ~31 days at 5s/ledger    |
| `BUMP_THRESHOLD` | `500_000` | Bump when remaining TTL is below |

**Policy:** every write to instance or persistent storage calls
`extend_ttl(BUMP_THRESHOLD, BUMP_TO)` on the affected key (or on the instance
entry for instance storage). Reads do not bump. There is no temporary storage
in use.

## The never-change-struct-shape rule

`#[contracttype]` structs are serialized with their field order and names baked
into the on-chain XDR. **Changing the shape of a struct that is already stored
in a deployed contract breaks deserialization of every existing entry** — the
contract will fail to read its own state after upgrade.

Rules:

1. Never add, remove, reorder, or rename fields on a struct that is stored in
   instance or persistent storage (e.g. `Market`, `Config`, `ReflectorConfig`,
   `RedstoneConfig`).
2. To add a field, introduce a **new storage key** holding the new value, and
   read it with a default when absent. This is why `DataKey::Paused` is its own
   key rather than a `Market` field.
3. Never change the discriminant or variant set of a stored enum in a way that
   renumbers existing variants.

### The `Option<CustomStruct>` gotcha

Soroban's `#[contracttype]` encoding of `Option<T>` for a custom struct `T` is
not stable across SDK versions in the way a plain struct is. Storing
`Option<ReflectorConfig>` / `Option<RedstoneConfig>` inline inside `Market`
would couple the market's deserialization to that encoding. Instead these live
under their own `DataKey` variants (`DataKey::Reflector`, `DataKey::Redstone`,
`DataKey::PriceOracle`) and are read as `Option` at the call site. Do not move
them back into `Market`.

## polaris-market (`contracts/market`)

Storage key enum: `DataKey`.

| Key                          | Class      | Value type              | Notes / TTL |
| ---------------------------- | ---------- | ----------------------- | ----------- |
| `DataKey::Market`            | instance   | `Market`                | Bumped with instance. **Never change `Market`'s shape.** |
| `DataKey::Config`            | instance   | `Config`                | Bumped with instance. |
| `DataKey::Paused`            | instance   | `bool`                  | Deliberately its own key, not a `Market` field, for upgrade compatibility. |
| `DataKey::Reflector`         | instance   | `Option<ReflectorConfig>` | Own key due to the `Option<CustomStruct>` gotcha. |
| `DataKey::Redstone`          | instance   | `Option<RedstoneConfig>`  | Own key due to the `Option<CustomStruct>` gotcha. |
| `DataKey::PriceOracle`       | instance   | `Option<Address>`       | Own key due to the `Option<CustomStruct>` gotcha. |
| `DataKey::Balance(Address)`   | persistent | `i128`                  | Per-holder share balance. Bumped on every write. |
| `DataKey::YesReserve`         | instance   | `i128`                  | AMM YES reserve. |
| `DataKey::NoReserve`          | instance   | `i128`                  | AMM NO reserve. |

## polaris-perpetual (`contracts/perpetual`)

Storage key enum: `DataKey`.

| Key                          | Class      | Value type              | Notes / TTL |
| ---------------------------- | ---------- | ----------------------- | ----------- |
| `DataKey::Config`            | instance   | `Config`                | Bumped with instance. **Never change `Config`'s shape.** |
| `DataKey::Paused`            | instance   | `bool`                  | Own key, same rationale as market. |
| `DataKey::Reflector`         | instance   | `Option<ReflectorConfig>` | Own key due to the `Option<CustomStruct>` gotcha. |
| `DataKey::Redstone`          | instance   | `Option<RedstoneConfig>`  | Own key due to the `Option<CustomStruct>` gotcha. |
| `DataKey::PriceOracle`       | instance   | `Option<Address>`       | Own key due to the `Option<CustomStruct>` gotcha. |
| `DataKey::Position(Address)` | persistent | `Position`              | Per-trader position. Bumped on every write. |
| `DataKey::Balance(Address)`  | persistent | `i128`                  | Per-holder collateral balance. Bumped on every write. |

## polaris-vault (`contracts/vault`)

Storage keys are `Symbol`s (via `symbol_short!`).

| Key (Symbol)   | Class      | Value type | Notes / TTL |
| -------------- | ---------- | ---------- | ----------- |
| `ADMIN`        | instance   | `Address`  | Bumped with instance. |
| `TOKEN`        | instance   | `Address`  | Underlying token. |
| `TOTAL`        | instance   | `i128`     | Total shares outstanding. |
| `BALANCE`      | persistent | `i128`     | Per-holder share balance, keyed by `(BALANCE, Address)`. Bumped on every write. |

## polaris-smart-wallet (`contracts/smart-wallet`)

Storage keys are `Symbol`s (via `symbol_short!`).

| Key (Symbol)   | Class      | Value type | Notes / TTL |
| -------------- | ---------- | ---------- | ----------- |
| `OWNER`        | instance   | `Address`  | Wallet owner. Bumped with instance. |
| `SIGNERS`      | instance   | `Vec<Address>` | Authorized signers. |
| `THRESHOLD`    | instance   | `u32`      | Required signatures. |
| `NONCE`        | instance   | `u64`      | Replay-protection nonce. |

## polaris-smart-wallet-factory (`contracts/smart-wallet-factory`)

Storage keys are `Symbol`s (via `symbol_short!`).

| Key (Symbol)   | Class      | Value type | Notes / TTL |
| -------------- | ---------- | ---------- | ----------- |
| `WASM`         | instance   | `Bytes`    | Wallet wasm hash used for deploys. Bumped with instance. |
| `COUNT`        | instance   | `u32`      | Number of wallets deployed. |

## Archival and restore planning

The persistent keys above are the ones that can be archived and must be
restored before they can be read again:

- `market`: `DataKey::Balance(Address)`
- `perpetual`: `DataKey::Position(Address)`, `DataKey::Balance(Address)`
- `vault`: `(BALANCE, Address)`

Instance storage is bumped together with the contract instance entry; if the
instance is archived, restoring the instance restores all instance keys.
