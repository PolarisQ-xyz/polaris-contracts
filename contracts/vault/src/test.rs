#![cfg(test)]
extern crate std;

use super::*;
use market_contract::Client as MarketClient;
use market_contract::OracleFeedConfig;
use polaris_mock_lazer::MockLazer;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Bytes, Env,
};

const FEED_ID: u32 = 100;
const DAY: u64 = 86_400;
const REFLECTOR_DECIMALS: u32 = 14; // matches the real Reflector testnet oracle, confirmed live
const REFLECTOR_RESOLUTION_SECS: u32 = 300;
const REFLECTOR_MAX_STALENESS_SECS: u64 = 600;
const REFLECTOR_TOLERANCE_BPS: u32 = 150;

/// Duplicated from `contracts/market/src/test.rs`'s own `mod mock_reflector`
/// — same "exact copy, nothing vault-specific" precedent already set by
/// `build_payload` below. This crate has no source dependency on
/// `polaris-market` (only a compiled-wasm import via `contractimport!`, see
/// the module doc on `market_contract` in `lib.rs`), so it can't share the
/// Rust type declaration directly; the wasm's own internal code is what
/// actually constructs `Asset`/`PriceData` when it calls `lastprice`, so
/// this only needs matching XDR shape (variant/field order), not identity.
mod mock_reflector {
    use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol};

    #[contracttype(export = false)]
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum Asset {
        Stellar(Address),
        Other(Symbol),
    }

    #[contracttype(export = false)]
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct PriceData {
        pub price: i128,
        pub timestamp: u64,
    }

    const PRICE_KEY: Symbol = symbol_short!("px");

    #[contract]
    pub struct MockReflector;

    #[contractimpl]
    impl MockReflector {
        pub fn set_price(env: Env, price: i128, timestamp: u64) {
            env.storage().instance().set(&PRICE_KEY, &(price, timestamp));
        }

        pub fn lastprice(env: Env, _asset: Asset) -> Option<PriceData> {
            env.storage()
                .instance()
                .get::<_, (i128, u64)>(&PRICE_KEY)
                .map(|(price, timestamp)| PriceData { price, timestamp })
        }

        pub fn decimals(_env: Env) -> u32 {
            super::REFLECTOR_DECIMALS
        }

        pub fn resolution(_env: Env) -> u32 {
            super::REFLECTOR_RESOLUTION_SECS
        }
    }
}

/// Wire-format payload matching pyth-lazer-stellar-sdk's parser — exact
/// copy of `contracts/market/src/test.rs`'s `build_payload`, since this is
/// the format `settle` needs and there's nothing vault-specific about it.
fn build_payload(feed_id: u32, price: i64, exponent: i16, feed_ts_micros: u64) -> std::vec::Vec<u8> {
    let mut b = std::vec::Vec::new();
    b.extend_from_slice(&2_479_346_549u32.to_le_bytes());
    b.extend_from_slice(&feed_ts_micros.to_le_bytes());
    b.push(3);
    b.push(1);
    b.extend_from_slice(&feed_id.to_le_bytes());
    b.push(3);
    b.push(0);
    b.extend_from_slice(&(price as u64).to_le_bytes());
    b.push(4);
    b.extend_from_slice(&(exponent as u16).to_le_bytes());
    b.push(12);
    b.push(1);
    b.extend_from_slice(&feed_ts_micros.to_le_bytes());
    b
}

struct Harness {
    env: Env,
    vault_id: Address,
    admin: Address,
    token_id: Address,
}

fn setup() -> Harness {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);

    let admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let token_id = sac.address();

    let vault_id = env.register(Vault, ());
    let client = VaultClient::new(&env, &vault_id);
    client.initialize(&admin, &token_id);

    Harness { env, vault_id, admin, token_id }
}

fn fund(h: &Harness, who: &Address, amount: i128) {
    token::StellarAssetClient::new(&h.env, &h.token_id).mint(who, &amount);
}

#[test]
fn deposit_and_withdraw_round_trip() {
    let h = setup();
    let lp = Address::generate(&h.env);
    fund(&h, &lp, 5_000);

    let client = VaultClient::new(&h.env, &h.vault_id);
    client.deposit(&lp, &1_000);
    assert_eq!(client.get_balance(), 1_000);
    assert_eq!(client.get_total_deposited(), 1_000);
    assert_eq!(token::Client::new(&h.env, &h.token_id).balance(&lp), 4_000);

    let recipient = Address::generate(&h.env);
    client.withdraw(&h.admin, &recipient, &600);
    assert_eq!(client.get_balance(), 400);
    assert_eq!(token::Client::new(&h.env, &h.token_id).balance(&recipient), 600);
}

#[test]
fn withdraw_requires_the_admins_own_authorization() {
    let h = setup();
    let lp = Address::generate(&h.env);
    fund(&h, &lp, 5_000);
    let client = VaultClient::new(&h.env, &h.vault_id);
    client.deposit(&lp, &1_000);

    let not_admin = Address::generate(&h.env);
    let result = client.try_withdraw(&not_admin, &lp, &100);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
    // Rejected means rejected — no state change at all.
    assert_eq!(client.get_balance(), 1_000);
}

#[test]
fn withdraw_beyond_balance_fails() {
    let h = setup();
    let lp = Address::generate(&h.env);
    fund(&h, &lp, 5_000);
    let client = VaultClient::new(&h.env, &h.vault_id);
    client.deposit(&lp, &1_000);

    let recipient = Address::generate(&h.env);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.withdraw(&h.admin, &recipient, &1_001)
    }));
    assert!(result.is_err(), "withdrawing more than the vault holds must fail, not succeed");
    assert_eq!(client.get_balance(), 1_000);
}

#[test]
fn deposit_before_initialize_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let token_id = sac.address();
    token::StellarAssetClient::new(&env, &token_id).mint(&admin, &1_000);

    let vault_id = env.register(Vault, ());
    let client = VaultClient::new(&env, &vault_id);
    let result = client.try_deposit(&admin, &100);
    assert_eq!(result, Err(Ok(Error::NotInitialized)));
}

#[test]
fn double_initialize_rejected() {
    let h = setup();
    let client = VaultClient::new(&h.env, &h.vault_id);
    let result = client.try_initialize(&h.admin, &h.token_id);
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
}

// ---------- admin rotation (#4) ----------

#[test]
fn propose_then_accept_rotates_admin() {
    let h = setup();
    let client = VaultClient::new(&h.env, &h.vault_id);
    let new_admin = Address::generate(&h.env);

    client.propose_admin(&h.admin, &new_admin);
    assert_eq!(client.get_pending_admin(), Some(new_admin.clone()));
    assert_eq!(client.get_admin(), h.admin); // not yet in effect

    client.accept_admin(&new_admin);
    assert_eq!(client.get_admin(), new_admin);
    assert_eq!(client.get_pending_admin(), None); // consumed

    // The OLD admin can no longer withdraw — proves the rotation actually
    // took effect, not just that the getters report a new value.
    let recipient = Address::generate(&h.env);
    let result = client.try_withdraw(&h.admin, &recipient, &1);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));

    // The NEW admin can.
    let lp = Address::generate(&h.env);
    fund(&h, &lp, 1_000);
    client.deposit(&lp, &1_000);
    client.withdraw(&new_admin, &recipient, &500);
    assert_eq!(client.get_balance(), 500);
}

#[test]
fn propose_admin_requires_the_current_admins_own_authorization() {
    let h = setup();
    let client = VaultClient::new(&h.env, &h.vault_id);
    let not_admin = Address::generate(&h.env);
    let new_admin = Address::generate(&h.env);
    let result = client.try_propose_admin(&not_admin, &new_admin);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
    assert_eq!(client.get_pending_admin(), None);
}

#[test]
fn accept_admin_rejects_an_address_that_was_never_proposed() {
    let h = setup();
    let client = VaultClient::new(&h.env, &h.vault_id);
    let impostor = Address::generate(&h.env);
    // No propose_admin call at all.
    let result = client.try_accept_admin(&impostor);
    assert_eq!(result, Err(Ok(Error::NoPendingAdmin)));
    assert_eq!(client.get_admin(), h.admin);
}

// ---------- admin proposal lifetime (#153) ----------

#[test]
fn expired_admin_proposal_is_rejected() {
    let h = setup();
    let client = VaultClient::new(&h.env, &h.vault_id);
    let new_admin = Address::generate(&h.env);

    client.propose_admin(&h.admin, &new_admin);
    assert_eq!(client.get_pending_admin(), Some(new_admin.clone()));

    // Jump past the 7-day proposal window.
    h.env.ledger().set_timestamp(h.env.ledger().timestamp() + 7 * DAY + 1);

    let result = client.try_accept_admin(&new_admin);
    assert_eq!(result, Err(Ok(Error::ProposalExpired)));
    // The stale proposal is cleared and the admin is unchanged.
    assert_eq!(client.get_pending_admin(), None);
    assert_eq!(client.get_admin(), h.admin);
}

#[test]
fn cancelled_admin_proposal_is_rejected() {
    let h = setup();
    let client = VaultClient::new(&h.env, &h.vault_id);
    let new_admin = Address::generate(&h.env);

    client.propose_admin(&h.admin, &new_admin);
    client.cancel_admin_proposal(&h.admin);
    assert_eq!(client.get_pending_admin(), None);

    let result = client.try_accept_admin(&new_admin);
    assert_eq!(result, Err(Ok(Error::NoPendingAdmin)));
    assert_eq!(client.get_admin(), h.admin);
}

#[test]
fn cancel_admin_proposal_requires_the_current_admin() {
    let h = setup();
    let client = VaultClient::new(&h.env, &h.vault_id);
    let new_admin = Address::generate(&h.env);
    client.propose_admin(&h.admin, &new_admin);

    let not_admin = Address::generate(&h.env);
    let result = client.try_cancel_admin_proposal(&not_admin);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
    // Proposal survives an unauthorized cancel attempt.
    assert_eq!(client.get_pending_admin(), Some(new_admin));
}
