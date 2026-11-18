#![no_std]

//! Shared CTF/AMM math and the SEP-40 oracle client, extracted from
//! `polaris-market` once `polaris-perpetual` needed the exact same pieces.
//! Deliberately `rlib`-only with no `#[contract]`/`#[contractimpl]` in this
//! crate at all (see `Cargo.toml`'s doc comment) — that's what makes it
//! safe for both contracts to depend on normally, unlike `polaris-vault`'s
//! `contractimport!` workaround for a *whole contract's* callable
//! interface, which this isn't.
//!
//! Every function here is pure (no `Env`, no storage access) except the
//! `sep40` module's on-chain client and `verify_oracle_corroboration`
//! (which only ever read). Nothing in this crate decides
//! fund-safety-critical outcomes on its own — each contract's own
//! `lib.rs` still owns every state mutation and every `require_auth()`;
//! this crate just guarantees the *math* and *verification logic* those
//! decisions are based on can't independently drift between contracts.

use core::ops::RangeInclusive;
use soroban_sdk::{contracttype, Env};

/// Persistent-entry TTL target (in ledgers) every Polaris contract extends
/// its instance/persistent storage to. Declared exactly once here so the
/// mainnet TTL policy can be tuned in one place instead of seven.
pub const BUMP_TO: u32 = 535_680;

/// Threshold below which a persistent entry is considered due for a bump.
pub const BUMP_THRESHOLD: u32 = 500_000;

/// Valid `decimals` range for a SEP-40 oracle feed, derived from the
/// conversion math in [`reflector_price_to_cents`]:
///
/// - Lower bound `2`: the conversion divides by `10^(decimals - 2)`, so
///   `decimals < 2` would underflow the exponent subtraction.
/// - Upper bound `30`: `10^30` is the largest power of ten that fits in an
///   `i128` (`i128::MAX ≈ 1.7 * 10^38`), so `10^(decimals - 2)` stays
///   representable up to `decimals == 30` and overflows beyond it.
///
/// Declared exactly once here so every caller (market, perpetual, oracle
/// corroboration) shares the same bounds instead of restating them.
pub const SEP40_DECIMALS_RANGE: RangeInclusive<u32> = 2..=30;

/// Returns `true` when `decimals` is within [`SEP40_DECIMALS_RANGE`].
pub fn valid_sep40_decimals(decimals: u32) -> bool {
    SEP40_DECIMALS_RANGE.contains(&decimals)
}

/// Extends the current contract instance's TTL to `BUMP_TO` when it has
/// fallen below `BUMP_THRESHOLD`. Shared by every Polaris contract so the
/// bump window can't drift between them (see #11, #132).
pub fn touch(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(BUMP_THRESHOLD, BUMP_TO);
}

/// amount_out for a constant-product swap of `amount_in` (post-fee) against
/// reserves (reserve_in, reserve_out). Floors, per Soroban i128 division.
pub fn cpmm_out(reserve_in: i128, reserve_out: i128, effective_in: i128) -> i128 {
    let k = reserve_in * reserve_out;
    let new_reserve_in = reserve_in + effective_in;
    let new_reserve_out = k / new_reserve_in;
    reserve_out - new_reserve_out
}

/// Floor integer square root via Newton's method (`n` assumed >= 0).
pub fn isqrt(n: i128) -> i128 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Collateral payout for selling `effective_in` (post-fee) `prediction`
/// shares straight to collateral in one call.
///
/// A naive "swap then merge" sell breaks whenever a user swaps away an
/// *entire* one-sided position: they end up holding pure opposite-side
/// shares with nothing left to merge against, and the trade silently
/// returns zero collateral. This solves the constant-product invariant
/// directly for the payout instead: find `x` (collateral out) such that
/// merging `x` of both sides out of the pool, after `effective_in` shares
/// were swapped in, preserves `k`:
///   (reserve_in + effective_in - x) * (reserve_out - x) == reserve_in * reserve_out
/// which is the quadratic `x^2 - x*S + P = 0` with `S = reserve_in +
/// effective_in + reserve_out`, `P = effective_in * reserve_out`; the
/// economically valid root is the smaller one, `x = (S - sqrt(S^2 - 4P)) / 2`.
/// This is the standard FPMM "sell" formula (as used by Gnosis's
/// conditional-token market makers), not something bespoke to this build.
pub fn cpmm_sell_out(reserve_in: i128, reserve_out: i128, effective_in: i128) -> i128 {
    let s = reserve_in + effective_in + reserve_out;
    let p = effective_in * reserve_out;
    let disc = s * s - 4 * p;
    let sqrt_disc = isqrt(core::cmp::max(disc, 0));
    (s - sqrt_disc) / 2
}

pub fn apply_fee(amount: i128, fee_bps: u32) -> i128 {
    let fee = amount * fee_bps as i128 / 10_000;
    amount - fee
}

/// Cost-driven fee curve: `base_fee_bps` at zero volume (`total_supply ==
/// initial_liquidity`), decaying toward `min_fee_bps` as `total_supply`
/// grows. Takes plain scalars rather than a contract-specific `Market`
/// struct (the original, pre-extraction shape) precisely so both
/// `polaris-market` and `polaris-perpetual` — which have differently-shaped
/// state structs — can share this one implementation.
///
/// `effective = min + (base - min) * initial_liquidity / total_supply`.
/// Well-defined and bounded to `[min_fee_bps, base_fee_bps]` as long as the
/// caller's own invariant `total_supply >= initial_liquidity` holds while
/// open (true in both contracts: every `merge`/`sell`/`redeem` can only
/// unwind collateral that `split`/`buy` actually locked; seed liquidity is
/// never itself withdrawable pre-resolution/pre-termination).
pub fn effective_fee_bps(base_fee_bps: u32, min_fee_bps: u32, initial_liquidity: i128, total_supply: i128) -> u32 {
    let base = base_fee_bps as i128;
    let min = min_fee_bps as i128;
    let scaled = min + (base - min) * initial_liquidity / total_supply;
    scaled as u32
}

/// Converts a Pyth raw price (`price * 10^exponent` = USD) into integer
/// cents (`USD * 100`), staying in integer math throughout.
pub fn to_cents(price: i64, exponent: i16) -> i128 {
    let p = price as i128;
    let e = exponent as i32 + 2;
    if e >= 0 {
        p * 10i128.pow(e as u32)
    } else {
        p / 10i128.pow((-e) as u32)
    }
}

/// Converts Reflector's `PriceData.price` (scaled by `decimals`, USD-based
/// same as Lazer's `to_cents`) into whole cents, rounding to the nearest
/// cent rather than truncating — a floor bias would skew every divergence
/// comparison in one direction. `decimals` must be within
/// [`SEP40_DECIMALS_RANGE`] (guards the `10^(decimals - 2)` scaling from
/// overflow/underflow at the extremes); debug-asserted here so a caller
/// that forgets to bounds-check trips in tests rather than silently
/// producing a wrong price.
pub fn reflector_price_to_cents(price: i128, decimals: u32) -> i128 {
    debug_assert!(
        valid_sep40_decimals(decimals),
        "decimals out of SEP-40 range"
    );
    let divisor = 10i128.pow(decimals - 2);
    let half = divisor / 2;
    if price >= 0 {
        (price + half) / divisor
    } else {
        -((-price + half) / divisor)
    }
}

/// Stellar's SEP-40 oracle interface
/// (https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0040.md),
/// copied verbatim rather than reimplemented — the pattern the ecosystem
/// uses for calling a third-party contract by known interface when
/// there's no local wasm to `contractimport!` against. Originally added
/// for Reflector Network; RedStone's Stellar SEP-40 wrapper contract
/// implements the exact same interface (confirmed live — see
/// `contracts/market`'s README section on both providers), so this is
/// generic to *any* SEP-40 oracle now, not Reflector-specific — hence the
/// module and client names, not `reflector`/`ReflectorPulseClient`.
///
/// Declared exactly once, here, for the whole workspace. A second,
/// independent declaration of a nominally-identical type would compile
/// fine but produce mutually-incompatible XDR, since Soroban's wire
/// encoding for a `contracttype` is structural (variant/field order), not
/// identity-based — this is precisely the trap `polaris-market`'s test
/// mock already avoided by reusing this module's types instead of
/// redeclaring them, before this crate existed to hold the canonical copy.
pub mod sep40 {
    use soroban_sdk::{contracttype, Address, Symbol};

    // The macro only needs the trait to generate `Sep40Client` — the
    // trait itself is never referenced by name elsewhere, hence `allow`.
    #[allow(dead_code)]
    #[soroban_sdk::contractclient(name = "Sep40Client")]
    pub trait Contract {
        fn lastprice(asset: Asset) -> Option<PriceData>;
        fn decimals() -> u32;
        fn resolution() -> u32;
    }

    // Plain `#[contracttype]`, not `export = false` — `Asset` used to only
    // describe an *external* contract's call shape (Reflector/RedStone's
    // `lastprice`), where suppressing its own top-level spec entry seemed
    // right. It's since become a genuine field of `OracleFeedConfig`,
    // which *is* part of this contract's own public interface (an
    // `i

/* … truncated 688 chars — edit only what you need near the top … */
