//! Dev-only shared test scaffolding for the Polaris contracts.
//!
//! Nothing here is production logic: it exists so `market`, `perpetual` and
//! `vault` tests stop carrying their own copies of the Lazer payload encoder,
//! the mock oracle contracts and the token-funding helper. `publish = false`
//! and an rlib-only crate type keep it out of every release wasm.

extern crate std;

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{token, Address, Env};

/// The mock oracle contracts, re-exported so tests have one import path.
pub use polaris_mock_lazer as mock_lazer;
pub use polaris_mock_redstone as mock_sep40;

/// Wire-format payload matching pyth-lazer-stellar-sdk's parser: magic(4) +
/// timestamp(8, LE µs) + channel(1) + num_feeds(1) + [feed_id(4) +
/// num_props(1) + properties...]. We only encode price(0), exponent(4), and
/// feed_update_timestamp(12) — the three properties `settle` reads.
pub fn build_payload(
    feed_id: u32,
    price: i64,
    exponent: i16,
    feed_ts_micros: u64,
) -> std::vec::Vec<u8> {
    let mut b = std::vec::Vec::new();
    b.extend_from_slice(&2_479_346_549u32.to_le_bytes()); // magic
    b.extend_from_slice(&feed_ts_micros.to_le_bytes()); // top-level timestamp
    b.push(3); // channel = FixedRate200ms
    b.push(1); // num_feeds
    b.extend_from_slice(&feed_id.to_le_bytes());
    b.push(3); // num_properties
    b.push(0); // property: price
    b.extend_from_slice(&(price as u64).to_le_bytes());
    b.push(4); // property: exponent
    b.extend_from_slice(&(exponent as u16).to_le_bytes());
    b.push(12); // property: feed_update_timestamp
    b.push(1); // exists = true
    b.extend_from_slice(&feed_ts_micros.to_le_bytes());
    b
}

/// Register a fresh Stellar asset contract and mint `amount` of it to
/// `to`. Returns the token's contract address.
pub fn fund(env: &Env, admin: &Address, to: &Address, amount: i128) -> Address {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let token_id = sac.address();
    token::StellarAssetClient::new(env, &token_id).mint(to, &amount);
    token_id
}

/// Create a fresh funded account: a random address holding `amount` of a new
/// asset. Returns `(holder, token_id)`.
pub fn funded_account(env: &Env, amount: i128) -> (Address, Address) {
    let admin = Address::generate(env);
    let holder = Address::generate(env);
    let token_id = fund(env, &admin, &holder, amount);
    (holder, token_id)
}
