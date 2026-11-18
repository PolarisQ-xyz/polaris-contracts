#![cfg(test)]
extern crate std;

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{token, Address, Env};

/// Same wire-format encoder as `polaris-market/src/test.rs` — duplicated
/// deliberately, not extracted: this is test-only scaffolding for a mock
/// payload format, not production logic, so there's no drift risk the
/// shared `polaris-ctf-math` crate exists to prevent.
fn build_payload(feed_id: u32, price: i64, exponent: i16, feed_ts_micros: u64) -> std::vec::Vec<u8> {
    let mut b = std::vec::Vec::new();
    b.extend_from_slice(&2_479_346_549u32.to_le_bytes()); // magic
    b.extend_from_slice(&feed_ts_micros.to_le_bytes());
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

const FEED_ID: u32 = 100;
const REFLECTOR_DECIMALS: u32 = 14;
const REFLECTOR_RESOLUTION_SECS: u32 = 300;
const REFLECTOR_MAX_STALENESS_SECS: u64 = 600;
const REFLECTOR_TOLERANCE_BPS: u32 = 150;

struct Harness {
    env: Env,
    market_id: Address,
    lazer_id: Address,
    reflector_id: Address,
    redstone_id: Address,
    token_id: Address,
    admin: Address,
    treasury: Address,
}

fn default_price_oracle(env: &Env, lazer_id: &Address, reflector_id: &Address, redstone_id: &Address) -> PriceOracleConfig {
    PriceOracleConfig {
        lazer_contract: lazer_id.clone(),
        feed_id: FEED_ID,
        reflector: OracleFeedConfig {
            contract: reflector_id.clone(),
            asset: polaris_ctf_math::sep40::Asset::Other(soroban_sdk::Symbol::new(env, "XLM")),
            max_staleness_secs: REFLECTOR_MAX_STALENESS_SECS,
            tolerance_bps: REFLECTOR_TOLERANCE_BPS,
        },
        // Overwritten live by `initialize` regardless of what's passed
        // here — see `PriceOracleConfig`'s doc comment.
        reflector_decimals_at_init: 0,
        redstone: OracleFeedConfig {
            contract: redstone_id.clone(),
            asset: polaris_ctf_math::sep40::Asset::Stellar(Address::generate(env)),
            max_staleness_secs: REFLECTOR_MAX_STALENESS_SECS,
            tolerance_bps: REFLECTOR_TOLERANCE_BPS,
        },
        redstone_decimals_at_init: 0,
    }
}

fn setup(initial_liquidity: i128, with_oracle: bool) -> Harness {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);

    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let token_id = sac.address();
    let token_admin = token::StellarAssetClient::new(&env, &token_id);

    let lazer_id = env.register(mock_lazer::MockLazer, ());
    let reflector_id = env.register(mock_sep40::MockSep40, ());
    let redstone_id = env.register(mock_sep40::MockSep40, ());
    let market_id = env.register(PolarisPerpetual, ());

    token_admin.mint(&admin, &(initial_liquidity * 1000));

    let price_oracle = if with_oracle {
        Some(default_price_oracle(&env, &lazer_id, &reflector_id, &redstone_id))
    } else {
        None
    };

    let client = PolarisPerpetualClient::new(&env, &market_id);
    client.initialize(&admin, &token_id, &100u32, &20u32, &treasury, &initial_liquidity, &price_oracle);

    Harness { env, market_id, lazer_id, reflector_id, redstone_id, token_id, admin, treasury }
}

fn fund(h: &Harness, who: &Address, amount: i128) {
    token::StellarAssetClient::new(&h.env, &h.token_id).mint(who, &amount);
}

fn set_reflector_price_cents(h: &Harness, cents: i128) {
    let client = mock_sep40::MockSep40Client::new(&h.env, &h.reflector_id);
    let price = cents * 10i128.pow(REFLECTOR_DECIMALS - 2);
    client.set_price(&price, &h.env.ledger().timestamp());
}

fn set_redstone_price_cents(h: &Harness, cents: i128) {
    let client = mock_sep40::MockSep40Client::new(&h.env, &h.redstone_id);
    let price = cents * 10i128.pow(REFLECTOR_DECIMALS - 2);
    client.set_price(&price, &h.env.ledger().timestamp());
}

mod mock_lazer {
    use soroban_sdk::{contract, contractimpl, Bytes, Env};

    #[contract]
    pub struct MockLazer;

    #[contractimpl]
    impl MockLazer {
        pub fn verify_update(_env: Env, data: Bytes) -> Bytes {
            data
        }
    }
}

/// A mock of the generic SEP-40 interface — reused for both the Reflector
/// and RedStone legs (two independent registered instances), since both
/// real providers implement the identical interface. See
/// `polaris-market/src/test.rs`'s identically-named module for the fuller
/// rationale.
mod mock_sep40 {
    use polaris_ctf_math::sep40::{Asset, PriceData};
    use soroban_sdk::{contract, contractimpl, symbol_short, Env};

    const PRICE_KEY: soroban_sdk::Symbol = symbol_short!("px");
    const DECIMALS_KEY: soroban_sdk::Symbol = symbol_short!("dec");
    const RESOLUTION_KEY: soroban_sdk::Symbol = symbol_short!("res");

    #[contract]
    pub struct MockSep40;

    #[contractimpl]
    impl MockSep40 {
        pub fn set_price(env: Env, price: i128, timestamp: u64) {
            env.storage().instance().set(&PRICE_KEY, &(price, timestamp));
        }

        pub fn set_none(env: Env) {
            env.storage().instance().remove(&PRICE_KEY);
        }

        pub fn set_decimals(env: Env, decimals: u32) {
            env.storage().instance().set(&DECIMALS_KEY, &decimals);
        }

        pub fn set_resolution(env: Env, resolution: u32) {
            env.storage().instance().set(&RESOLUTION_KEY, &resolution);
        }

        pub fn lastprice(env: Env, _asset: Asset) -> Option<PriceData> {
            env.storage().instance().get::<_, (i128, u64)>(&PRICE_KEY).map(|(price, timestamp)| PriceData { price, timestamp })
        }

        pub fn decimals(env: Env) -> u32 {
            env.storage().instance().get(&DECIMALS_KEY).unwrap_or(super::REFLECTOR_DECIMALS)
        }

        pub fn resolution(env: Env) -> u32 {
            env.storage().instance().get(&RESOLUTION_KEY).unwrap_or(super::REFLECTOR_RESOLUTION_SECS)
        }
    }
}

fn assert_solvent(h: &Harness) {
    let client = PolarisPerpetualClient::new(&h.env, &h.market_id);
    let m = client.get_market();
    let bal = token::Client::new(&h.env, &h.token_id).balance(&h.market_id);
    assert_eq!(bal, m.total_supply as i128, "collateral balance must always equal total_supply");
}

// ---------- initialize ----------

#[test]
fn init_seeds_pool_and_pulls_liquidity_with_no_strike_or_expiry() {
    let h = setup(10_000, true);
    let client = PolarisPerpetualClient::new(&h.env, &h.market_id);
    let m = client.get_market();
    assert_eq!(m.pool_yes, 10_000);
    assert_eq!(m.pool_no, 10_000);
    assert_eq!(m.total_supply, 10_000);
    assert_eq!(m.status, PerpetualStatus::Open);
    assert_eq!(token::Client::new(&h.env, &h.token_id).balance(&h.market_id), 10_000);
}

#[test]
fn init_without_a_price_oracle_is_allowed() {
    let h = setup(10_000, false);
    let client = PolarisPerpetualClient::new(&h.env, &h.market_id);
    assert_eq!(client.get_price_oracle(), None);
}

#[test]
fn double_initialize_rejected() {
    let h = setup(10_000, true);
    let client = PolarisPerpetualClient::new(&h.env, &h.market_id);
    let res = client.try_initialize(
        &h.admin, &h.token_id, &100u32, &20u32, &h.treasury, &10_000i128,
        &Some(default_price_oracle(&h.env, &h.lazer_id, &h.reflector_id, &h.redstone_id)),
    );
    assert_eq!(res, Err(Ok(Error::AlreadyInitialized)));
}

// ---------- split / merge / buy / sell — identical mechanics to polaris-market ----------

#[test]
fn split_then_merge_round_trips_collateral() {
    let h = setup(10_000, true);
    let client = PolarisPerpetualClient::new(&h.env, &h.market_id);
    let user = Address::generate(&h.env);
    fund(&h, &user, 1_000);

    client.split(&user, &1_000i128);
    assert_eq!(client.get_position(&user).yes, 1_000);
    assert_eq!(client.get_position(&user).no, 1_000);

    client.merge(&user, &1_000i128);
    assert_eq!(client.get_position(&user).yes, 0);
    assert_eq!(client.get_position(&user).no, 0);
    assert_eq!(token::Client::new(&h.env, &h.token_id).balance(&user), 1_000);
    assert_solvent(&h);
}

// ---------- parity with polaris-market ----------

#[test]
fn sell_large_enough_to_exhaust_a_reserve_is_rejected() {
    let h = setup(10_000, true);
    let client = PolarisPerpetualClient::new(&h.env, &h.market_id);
    let user = Address::generate(&h.env);
    fund(&h, &user, 10_000);

    client.split(&user, &10_000i128);
    // Selling more YES than the pool holds must be rejected by the
    // pool-depth guard rather than draining the reserve.
    let res = client.try_sell(&user, &Side::Yes, &20_000i128, &0i128);
    assert!(res.is_err(), "sell exceeding pool depth must be rejected");
    assert_solvent(&h);
}

#[test]
fn buy_then_sell_returns_close_to_original_minus_fees() {
    let h = setup(10_000, true);
    let client = PolarisPerpetualClient::new(&h.env, &h.market_id);
    let user = Address::generate(&h.env);
    fund(&h, &user, 1_000);

    let start = token::Client::new(&h.env, &h.token_id).balance(&user);
    client.buy(&user, &Side::Yes, &100i128, &1_000i128);
    let yes = client.get_position(&user).yes;
    assert!(yes > 0);

    client.sell(&user, &Side::Yes, &yes, &0i128);
    let end = token::Client::new(&h.env, &h.token_id).balance(&user);

    // Round trip should return close to the original, minus fees and
    // any small rounding — never more than the starting balance.
    assert!(end <= start, "round trip must not create collateral");
    assert!(end >= start - 10, "round trip should return close to original minus fees");
    assert_solvent(&h);
}

#[test]
fn fee_never_exceeds_base_even_if_total_supply_dips_back_toward_seed() {
    let h = setup(10_000, true);
    let client = PolarisPerpetualClient::new(&h.env, &h.market_id);
    let user = Address::generate(&h.env);
    fund(&h, &user, 10_000);

    // Grow the pool, then shrink it back toward the seed size.
    client.buy(&user, &Side::Yes, &5_000i128, &10_000i128);
    let yes = client.get_position(&user).yes;
    client.sell(&user, &Side::Yes, &yes, &0i128);

    // A subsequent trade's fee must still be bounded by the base fee.
    let before = token::Client::new(&h.env, &h.token_id).balance(&user);
    client.buy(&user, &Side::No, &100i128, &1_000i128);
    let after = token::Client::new(&h.env, &h.token_id).balance(&user);
    let spent = before - after;
    assert!(spent >= 100, "must spend at least the requested amount");
    assert!(spent <= 100 + 100, "fee must never exceed the base fee");
    assert_solvent(&h);
}

#[test]
fn normal_sized_trades_are_unaffected_by_the_pool_depth_guard() {
    let h = setup(10_000, true);
    let client = PolarisPerpetualClient::new(&h.env, &h.market_id);
    let user = Address::generate(&h.env);
    fund(&h, &user, 1_000);

    // A normal-sized buy and sell should both succeed.
    client.buy(&user, &Side::Yes, &100i128, &1_000i128);
    let yes = client.get_position(&user).yes;
    assert!(yes > 0);
    client.sell(&user, &Side::Yes, &yes, &0i128);
    assert_solvent(&h);
}

#[test]
fn fee_only_taken_on_swaps_never_on_split_merge_or_terminate() {
    let h = setup(10_000, true);
    let client = PolarisPerpetualClient::new(&h.env, &h.market_id);
    let user = Address::generate(&h.env);
    fund(&h, &user, 1_000);

    // split / merge must not charge a fee.
    let before = token::Client::new(&h.env, &h.token_id).balance(&user);
    client.split(&user, &1_000i128);
    client.merge(&user, &1_000i128);
    let after = token::Client::new(&h.env, &h.token_id).balance(&user);
    assert_eq!(before, after, "split/merge must not charge a fee");

    // terminate must not charge a fee either.
    let before = token::Client::new(&h.env, &h.token_id).balance(&user);
    client.terminate(&h.admin);
    let after = token::Client::new(&h.env, &h.token_id).balance(&user);
    assert_eq!(before, after, "terminate must not charge a fee");
    assert_solvent(&h);
}
