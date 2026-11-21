#![no_std]

//! Typed clients and contract types for the Polaris contracts, generated
//! from each contract's released wasm — import this crate instead of
//! hand-copying a `.wasm` file into your own `contractimport!`.
//!
//! Each module is one contract's `contractimport!` output: a `Client`
//! (plus `try_*` variants), every `#[contracttype]` its interface uses,
//! its `Error` enum, and `WASM` (the release bytes, for deploying a real
//! instance in your own tests). Modules are independent, so same-named
//! types — `market::OracleFeedConfig` and `perpetual::OracleFeedConfig` —
//! don't collide; they share an XDR shape, not a Rust type.
//!
//! ```
//! use polaris_interface::market;
//! use soroban_sdk::{Address, Env};
//!
//! /// The YES side's current AMM price, in basis points.
//! fn yes_price_bps(env: &Env, market_id: &Address) -> u32 {
//!     market::Client::new(env, market_id).get_price().0
//! }
//! ```
//!
//! This crate's version is the contract release its wasm came from; see
//! `README.md` for the release process and what is checked in CI.

/// `polaris-market`: the fixed-expiry binary market.
// The generated `initialize` client mirrors the contract's own signature,
// which carries the same allow at its source.
#[allow(clippy::too_many_arguments)]
pub mod market {
    soroban_sdk::contractimport!(file = "wasm/polaris_market.wasm");
}

/// `polaris-perpetual`: the no-expiry binary market.
#[allow(clippy::too_many_arguments)]
pub mod perpetual {
    soroban_sdk::contractimport!(file = "wasm/polaris_perpetual.wasm");
}

/// `polaris-vault`: LP capital custody, including `redeem_from_markets`.
pub mod vault {
    soroban_sdk::contractimport!(file = "wasm/polaris_vault.wasm");
}

/// `polaris-smart-wallet`: the passkey wallet `smart_wallet_factory`
/// deploys. Its `WASM` is what to upload before initializing a factory.
pub mod smart_wallet {
    // The generated client for `__check_auth` names `Context` unqualified.
    use soroban_sdk::auth::Context;
    soroban_sdk::contractimport!(file = "wasm/polaris_smart_wallet.wasm");
}

/// `polaris-smart-wallet-factory`: deploys and resolves smart wallets.
pub mod smart_wallet_factory {
    soroban_sdk::contractimport!(file = "wasm/polaris_smart_wallet_factory.wasm");
}

#[cfg(test)]
mod test;
