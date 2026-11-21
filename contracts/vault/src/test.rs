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
    // No propose_admin call at all — NoPendingAdmin, not Unauthorized,
    // distinguishes "nothing to accept" from "wrong address accepting".
    let result = client.try_accept_admin(&impostor);
    assert_eq!(result, Err(Ok(Error::NoPendingAdmin)));
}

#[test]
fn accept_admin_rejects_a_different_address_than_the_one_proposed() {
    let h = setup();
    let client = VaultClient::new(&h.env, &h.vault_id);
    let proposed = Address::generate(&h.env);
    let impostor = Address::generate(&h.env);
    client.propose_admin(&h.admin, &proposed);

    let result = client.try_accept_admin(&impostor);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
    // Rejected means rejected — admin is still unchanged, proposal intact.
    assert_eq!(client.get_admin(), h.admin);
    assert_eq!(client.get_pending_admin(), Some(proposed));
}

#[test]
fn a_bad_proposal_is_harmless_and_can_be_overwritten() {
    // The whole point of two-step rotation: proposing a typo'd/unreachable
    // address never bricks anything, since accept_admin always requires
    // that exact address's own live signature before anything changes.
    let h = setup();
    let client = VaultClient::new(&h.env, &h.vault_id);
    let typo = Address::generate(&h.env);
    let correct = Address::generate(&h.env);

    client.propose_admin(&h.admin, &typo);
    // Re-propose with the correct address — overwrites the bad one outright.
    client.propose_admin(&h.admin, &correct);
    assert_eq!(client.get_pending_admin(), Some(correct.clone()));

    client.accept_admin(&correct);
    assert_eq!(client.get_admin(), correct);
}

// ---------- the integration this vault exists for: fund a market's seed
// liquidity, collect the payout back after settlement ----------

#[test]
fn redeem_from_market_collects_the_vaults_treasury_payout() {
    let h = setup();
    let lp = Address::generate(&h.env);
    fund(&h, &lp, 1_000_000);
    let vault_client = VaultClient::new(&h.env, &h.vault_id);
    vault_client.deposit(&lp, &10_000);

    // The vault funds a market's initial_liquidity — exactly the
    // MarketFactoryService flow this contract was built for: withdraw seed
    // capital from the vault to the admin (who deploys/initializes the
    // market, same as StellarService.deployMarket does today), and the
    // market's own treasury is set to the vault itself.
    vault_client.withdraw(&h.admin, &h.admin, &10_000);

    let lazer_id = h.env.register(MockLazer, ());
    let reflector_id = h.env.register(mock_reflector::MockReflector, ());
    let market_id = h.env.register(market_contract::WASM, ());
    let market_client = MarketClient::new(&h.env, &market_id);
    let expiry = h.env.ledger().timestamp() + DAY;

    market_client.initialize(
        &h.admin,
        &h.token_id,
        &1_000_000i128, // strike: $10,000.00 in cents
        &expiry,
        &3_600u64,
        &lazer_id,
        &FEED_ID,
        &100u32,
        &20u32,
        &h.vault_id, // treasury = this vault, not a bare wallet
        &10_000i128,
        &OracleFeedConfig {
            contract: reflector_id.clone(),
            asset: market_contract::Asset::Other(soroban_sdk::Symbol::new(&h.env, "XLM")),
            max_staleness_secs: REFLECTOR_MAX_STALENESS_SECS,
            tolerance_bps: REFLECTOR_TOLERANCE_BPS,
        },
        &None, // no RedStone leg — this test exercises testnet-shaped behavior
    );

    // No trading — the simplest case: the market's entire pool (all of the
    // vault's seed liquidity) is what the vault should get back.
    h.env.ledger().set_timestamp(expiry);
    let payload = build_payload(FEED_ID, 2_000_000, -2, expiry * 1_000_000); // price $20,000 >= $10,000 strike -> ResolvedYes
    let reflector_client = mock_reflector::MockReflectorClient::new(&h.env, &reflector_id);
    reflector_client.set_price(&(2_000_000i128 * 10i128.pow(REFLECTOR_DECIMALS - 2)), &expiry); // agrees with Lazer's $20,000
    market_client.settle(&Bytes::from_slice(&h.env, &payload));

    let payout = vault_client.redeem_from_market(&h.admin, &market_id);
    assert_eq!(payout, 10_000, "the vault gets back exactly the seed liquidity it funded, at par, no trading occurred");
    assert_eq!(vault_client.get_balance(), 10_000);
}

// ---------- batch redemption (#38) ----------

/// Deploys a real `polaris-market` (from its compiled wasm, so metering
/// below includes VM instantiation) seeded with `liquidity` from the
/// admin, `treasury` = this vault, settled `ResolvedYes`. With no trading,
/// the vault's whole claim is the seed: it should redeem exactly
/// `liquidity`.
fn settled_market(h: &Harness, liquidity: i128) -> Address {
    fund(h, &h.admin, liquidity);
    let lazer_id = h.env.register(MockLazer, ());
    let reflector_id = h.env.register(mock_reflector::MockReflector, ());
    let market_id = h.env.register(market_contract::WASM, ());
    let market_client = MarketClient::new(&h.env, &market_id);
    let expiry = h.env.ledger().timestamp() + DAY;
    market_client.initialize(
        &h.admin,
        &h.token_id,
        &1_000_000i128,
        &expiry,
        &3_600u64,
        &lazer_id,
        &FEED_ID,
        &100u32,
        &20u32,
        &h.vault_id,
        &liquidity,
        &OracleFeedConfig {
            contract: reflector_id.clone(),
            asset: market_contract::Asset::Other(soroban_sdk::Symbol::new(&h.env, "XLM")),
            max_staleness_secs: REFLECTOR_MAX_STALENESS_SECS,
            tolerance_bps: REFLECTOR_TOLERANCE_BPS,
        },
        &None,
    );
    let now = h.env.ledger().timestamp();
    h.env.ledger().set_timestamp(expiry);
    let payload = build_payload(FEED_ID, 2_000_000, -2, expiry * 1_000_000);
    mock_reflector::MockReflectorClient::new(&h.env, &reflector_id)
        .set_price(&(2_000_000i128 * 10i128.pow(REFLECTOR_DECIMALS - 2)), &expiry);
    market_client.settle(&Bytes::from_slice(&h.env, &payload));
    // Rewind so the next market's `expiry` is still in the future.
    h.env.ledger().set_timestamp(now);
    market_id
}

#[test]
fn redeem_from_markets_collects_every_market_in_one_call() {
    let h = setup();
    let vault = VaultClient::new(&h.env, &h.vault_id);
    let a = settled_market(&h, 10_000);
    let b = settled_market(&h, 25_000);
    let c = settled_market(&h, 7_000);

    let outcomes = vault.redeem_from_markets(&h.admin, &soroban_sdk::vec![&h.env, a, b, c]);
    assert_eq!(
        outcomes,
        soroban_sdk::vec![
            &h.env,
            RedeemOutcome::Redeemed(10_000),
            RedeemOutcome::Redeemed(25_000),
            RedeemOutcome::Redeemed(7_000),
        ]
    );
    assert_eq!(vault.get_balance(), 42_000);
}

#[test]
fn one_failing_market_does_not_revert_the_rest_of_the_batch() {
    let h = setup();
    let vault = VaultClient::new(&h.env, &h.vault_id);
    let good = settled_market(&h, 10_000);
    let already_collected = settled_market(&h, 5_000);
    vault.redeem_from_market(&h.admin, &already_collected);
    assert_eq!(vault.get_balance(), 5_000);

    // Still open: initialized, never settled.
    fund(&h, &h.admin, 1_000);
    let open_id = h.env.register(market_contract::WASM, ());
    MarketClient::new(&h.env, &open_id).initialize(
        &h.admin,
        &h.token_id,
        &1_000_000i128,
        &(h.env.ledger().timestamp() + DAY),
        &3_600u64,
        &h.env.register(MockLazer, ()),
        &FEED_ID,
        &100u32,
        &20u32,
        &h.vault_id,
        &1_000i128,
        &OracleFeedConfig {
            contract: h.env.register(mock_reflector::MockReflector, ()),
            asset: market_contract::Asset::Other(soroban_sdk::Symbol::new(&h.env, "XLM")),
            max_staleness_secs: REFLECTOR_MAX_STALENESS_SECS,
            tolerance_bps: REFLECTOR_TOLERANCE_BPS,
        },
        &None,
    );
    let not_a_contract = Address::generate(&h.env);
    let second_good = settled_market(&h, 3_000);

    let outcomes = vault.redeem_from_markets(
        &h.admin,
        &soroban_sdk::vec![&h.env, good, already_collected, open_id.clone(), not_a_contract, second_good],
    );
    assert_eq!(
        outcomes,
        soroban_sdk::vec![
            &h.env,
            RedeemOutcome::Redeemed(10_000),
            RedeemOutcome::MarketError(market_contract::Error::NothingToRedeem as u32),
            RedeemOutcome::MarketError(market_contract::Error::NotFinalized as u32),
            RedeemOutcome::Failed,
            RedeemOutcome::Redeemed(3_000),
        ]
    );
    // Both successes committed despite three failures between them.
    assert_eq!(vault.get_balance(), 5_000 + 10_000 + 3_000);
    // The open market's failed sub-call left it untouched.
    assert_eq!(MarketClient::new(&h.env, &open_id).get_market().total_supply, 1_000);
}

#[test]
fn redeem_from_markets_requires_the_admins_own_authorization() {
    let h = setup();
    let vault = VaultClient::new(&h.env, &h.vault_id);
    let m = settled_market(&h, 10_000);
    let not_admin = Address::generate(&h.env);
    let result = vault.try_redeem_from_markets(&not_admin, &soroban_sdk::vec![&h.env, m]);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
    assert_eq!(vault.get_balance(), 0);
}

#[test]
fn redeem_from_markets_rejects_an_oversized_batch_before_calling_any_market() {
    let h = setup();
    let vault = VaultClient::new(&h.env, &h.vault_id);
    let m = settled_market(&h, 10_000);
    let mut ids = soroban_sdk::Vec::new(&h.env);
    for _ in 0..=MAX_REDEEM_BATCH {
        ids.push_back(m.clone());
    }
    let result = vault.try_redeem_from_markets(&h.admin, &ids);
    assert_eq!(result, Err(Ok(Error::BatchTooLarge)));
    // Nothing redeemed: the market's own claim is still collectable.
    assert_eq!(vault.redeem_from_market(&h.admin, &m), 10_000);
}

#[test]
fn redeem_from_markets_with_an_empty_list_is_a_no_op() {
    let h = setup();
    let vault = VaultClient::new(&h.env, &h.vault_id);
    assert_eq!(vault.redeem_from_markets(&h.admin, &soroban_sdk::Vec::new(&h.env)).len(), 0);
}

/// `n` settled markets, then one `redeem_from_markets` over all of them,
/// returning the resources that call metered. The host's test-only
/// *shadow* budget (auth-tree recording and event sizing for test
/// observation; free and unmetered on a real network) is lifted first:
/// it trips at around 10 nested market calls and then reports a harness
/// artifact — a panic in the auth-recording hook, or `0` event bytes —
/// rather than a network limit. The network limits themselves stay
/// enforced unless `enforce_mainnet_limits` is false.
struct Metered {
    footprint_entries: u32,
    write_entries: u32,
    instructions: i64,
    mem_bytes: i64,
    write_bytes: u32,
    event_bytes: u32,
}

fn metered_batch(n: u32, enforce_mainnet_limits: bool) -> (Vec<RedeemOutcome>, Metered) {
    let h = setup();
    if !enforce_mainnet_limits {
        h.env.cost_estimate().disable_resource_limits();
    }
    let vault = VaultClient::new(&h.env, &h.vault_id);
    let mut ids = soroban_sdk::Vec::new(&h.env);
    for _ in 0..n {
        ids.push_back(settled_market(&h, 10_000));
    }
    h.env.cost_estimate().budget().reset_unlimited();
    let outcomes = vault.redeem_from_markets(&h.admin, &ids);
    let r = h.env.cost_estimate().resources();
    let metered = Metered {
        // The SDK's own footprint model (what `ledger_entries` enforces).
        footprint_entries: r.disk_read_entries + r.memory_read_entries + r.write_entries,
        write_entries: r.write_entries,
        instructions: r.instructions,
        mem_bytes: r.mem_bytes,
        write_bytes: r.write_bytes,
        event_bytes: r.contract_events_size_bytes,
    };
    (outcomes, metered)
}

#[test]
fn a_full_size_batch_fits_mainnet_transaction_limits() {
    // The SDK enforces `InvocationResourceLimits::mainnet()` on every
    // invocation by default, so this call itself panics if a full batch
    // ever stops fitting. The explicit asserts below restate the limits
    // that matter so a regression names the one it broke.
    let (outcomes, res) = metered_batch(MAX_REDEEM_BATCH, true);
    assert_eq!(outcomes.len(), MAX_REDEEM_BATCH);
    for outcome in outcomes.iter() {
        assert_eq!(outcome, RedeemOutcome::Redeemed(10_000));
    }
    assert!(res.footprint_entries <= 100, "footprint entries {} > 100", res.footprint_entries);
    assert!(res.write_entries <= 50, "write entries {} > 50", res.write_entries);
    // Also under the older 100M per-transaction instruction limit, not
    // just today's 600M.
    assert!(res.instructions <= 100_000_000, "instructions {}", res.instructions);
    assert!(res.mem_bytes <= 41_943_040, "memory {}", res.mem_bytes);
    assert!(res.event_bytes <= 16_384, "event bytes {}", res.event_bytes);
}

/// Measurement harness behind `MAX_REDEEM_BATCH`'s table, not a
/// regression test: `cargo test -p polaris-vault measure_redeem_batch --
/// --ignored --nocapture` reprints it. Runs with mainnet limits disabled
/// so a size past the ceiling is measured instead of panicking; to
/// measure past `MAX_REDEEM_BATCH` itself (as the table's 11 and 12 rows
/// were), raise the constant temporarily, since the cap rejects larger
/// batches before calling any market.
#[test]
#[ignore]
fn measure_redeem_batch_costs() {
    for n in 1..=MAX_REDEEM_BATCH {
        let (_, res) = metered_batch(n, false);
        std::println!(
            "n={n:2} footprint={:>3} write_entries={:>3} instructions={:>11} mem={:>9} write_bytes={:>6} event_bytes={:>5}",
            res.footprint_entries,
            res.write_entries,
            res.instructions,
            res.mem_bytes,
            res.write_bytes,
            res.event_bytes,
        );
    }
}
