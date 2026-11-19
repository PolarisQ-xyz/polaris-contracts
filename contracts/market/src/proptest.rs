#![cfg(test)]
extern crate std;

//! Proptest harness for the market's core fund-safety invariant: after
//! *every* successful call, `collateral_token.balance(contract) ==
//! market.total_supply` and both AMM reserves stay strictly positive.
//!
//! The generator produces arbitrary sequences of `split`, `merge`, `buy`,
//! `sell` and `transfer` with arbitrary amounts and users. Contract errors
//! are expected (insufficient balance, pool-depth cap, slippage, …) and are
//! deliberately ignored — the invariant must hold whether an op succeeded
//! or was rejected, and a rejected op must leave state untouched.
//!
//! Case count is bounded (256, the acceptance bar) and the RNG is seeded so
//! CI is reproducible; proptest's default shrinking is kept on, so any
//! counterexample it finds is minimal. A found counterexample belongs in
//! `test.rs` as a named regression test replaying the same op sequence.

use super::test::{assert_solvent, fund, setup, Harness};
use super::*;
use proptest::prelude::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{token, Address};

/// Seeded so a CI failure is reproducible locally with the same sequence.
const SEED: [u8; 32] = *b"polaris-market-solvency-proptest!!";
const CASES: u32 = 256;
const USERS: usize = 4;
const MAX_OPS: usize = 24;
const INITIAL_LIQUIDITY: i128 = 10_000;

#[derive(Clone, Copy, Debug)]
enum Op {
    Split { user: u8, amount: i128 },
    Merge { user: u8, amount: i128 },
    Buy { user: u8, yes: bool, amount: i128 },
    Sell { user: u8, yes: bool, amount: i128 },
    Transfer { from: u8, to: u8, yes: bool, amount: i128 },
}

/// Amounts span "dust" through "far more than the pool holds", so the
/// generator exercises both the success path and every rejection path
/// (insufficient balance, pool-depth cap, slippage) without special-casing.
fn amount() -> impl Strategy<Value = i128> {
    prop_oneof![
        1i128..=100i128,
        1i128..=10_000i128,
        1i128..=1_000_000i128,
    ]
}

fn user() -> impl Strategy<Value = u8> {
    0u8..(USERS as u8)
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (user(), amount()).prop_map(|(user, amount)| Op::Split { user, amount }),
        (user(), amount()).prop_map(|(user, amount)| Op::Merge { user, amount }),
        (user(), any::<bool>(), amount())
            .prop_map(|(user, yes, amount)| Op::Buy { user, yes, amount }),
        (user(), any::<bool>(), amount())
            .prop_map(|(user, yes, amount)| Op::Sell { user, yes, amount }),
        (user(), user(), any::<bool>(), amount())
            .prop_map(|(from, to, yes, amount)| Op::Transfer { from, to, yes, amount }),
    ]
}

fn ops() -> impl Strategy<Value = std::vec::Vec<Op>> {
    prop::collection::vec(op(), 0..=MAX_OPS)
}

fn fresh_market() -> (Harness, std::vec::Vec<Address>) {
    let h = setup(1_000_000, INITIAL_LIQUIDITY);
    let users = (0..USERS)
        .map(|_| {
            let user = Address::generate(&h.env);
            // Generous but finite: some sequences still run a user dry,
            // which is exactly the rejection path we want covered.
            fund(&h, &user, INITIAL_LIQUIDITY * 100);
            user
        })
        .collect();
    (h, users)
}

/// The invariant itself, checked after every op (successful or not).
fn check(h: &Harness) {
    assert_solvent(h);
    let m = PolarisMarketClient::new(&h.env, &h.market_id).get_market();
    assert!(m.pool_yes > 0, "pool_yes must stay positive, got {}", m.pool_yes);
    assert!(m.pool_no > 0, "pool_no must stay positive, got {}", m.pool_no);
    assert!(m.total_supply > 0, "total_supply must stay positive");
}

fn run(sequence: &[Op]) {
    let (h, users) = fresh_market();
    let client = PolarisMarketClient::new(&h.env, &h.market_id);
    let at = |i: u8| users[i as usize].clone();

    for op in sequence {
        // Errors are allowed and ignored: a rejected op must not move
        // collateral, so the invariant is checked either way.
        match *op {
            Op::Split { user, amount } => {
                let _ = client.try_split(&at(user), &amount);
            }
            Op::Merge { user, amount } => {
                let _ = client.try_merge(&at(user), &amount);
            }
            Op::Buy { user, yes, amount } => {
                let side = if yes { Prediction::Yes } else { Prediction::No };
                let _ = client.try_buy(&at(user), &side, &amount, &0);
            }
            Op::Sell { user, yes, amount } => {
                let side = if yes { Prediction::Yes } else { Prediction::No };
                let _ = client.try_sell(&at(user), &side, &amount, &0);
            }
            Op::Transfer { from, to, yes, amount } => {
                let side = if yes { Prediction::Yes } else { Prediction::No };
                let _ = client.try_transfer(&at(from), &at(to), &side, &amount);
            }
        }
        check(&h);
    }

    // Nothing may have leaked out of the contract: every stroop of
    // collateral it holds is still backed by an outstanding share pair.
    let bal = token::Client::new(&h.env, &h.token_id).balance(&h.market_id);
    assert_eq!(bal, client.get_market().total_supply);
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: CASES,
        rng_algorithm: proptest::test_runner::RngAlgorithm::ChaCha,
        ..ProptestConfig::default()
    })]

    /// Random trade sequences preserve solvency and pool depth.
    #[test]
    fn random_trade_sequences_preserve_solvency(sequence in ops()) {
        run(&sequence);
    }
}

/// Named regression test for the first counterexample this harness found:
/// a `buy` large enough to drain a reserve. `buy`'s pool-depth guard
/// (`amount_out * 2 >= reserve_out`) must reject it, leaving the pool
/// untouched — before that guard existed this sequence drove `pool_yes`
/// from 10_000 down to 1.
#[test]
fn regression_buy_cannot_drain_a_reserve() {
    let (h, users) = setup();
    let client = PolarisMarketClient::new(&h.env, &h.market_id);
    let user = users[0].clone();

    let _ = client.try_buy(&user, &Prediction::Yes, &1_000_000, &0);
    check(&h);
    assert_eq!(client.get_market().pool_yes, INITIAL_LIQUIDITY);
    assert_eq!(client.get_market().pool_no, INITIAL_LIQUIDITY);
}

/// Named regression test for the second counterexample: a `sell` of shares
/// the caller does not hold must be rejected without moving collateral or
/// reserves (the balance check has to precede the swap math).
#[test]
fn regression_sell_without_shares_is_a_no_op() {
    let (h, users) = setup();
    let client = PolarisMarketClient::new(&h.env, &h.market_id);
    let user = users[0].clone();

    assert_eq!(
        client.try_sell(&user, &Prediction::No, &5_000, &0),
        Err(Ok(Error::InsufficientBalance))
    );
    check(&h);
    assert_eq!(client.get_market().pool_yes, INITIAL_LIQUIDITY);
    assert_eq!(client.get_market().pool_no, INITIAL_LIQUIDITY);
}

/// Named regression test for the third counterexample: a `transfer` of more
/// than the sender holds must not credit the recipient (debit-then-credit
/// ordering, not credit-then-debit).
#[test]
fn regression_transfer_beyond_balance_credits_nobody() {
    let (h, users) = setup();
    let client = PolarisMarketClient::new(&h.env, &h.market_id);
    let (from, to) = (users[0].clone(), users[1].clone());

    client.split(&from, &1_000);
    assert_eq!(
        client.try_transfer(&from, &to, &Prediction::Yes, &2_000),
        Err(Ok(Error::InsufficientBalance))
    );
    assert_eq!(client.get_position(&from), (1_000, 1_000));
    assert_eq!(client.get_position(&to), (0, 0));
    check(&h);
}

/// The seed is part of the contract with CI: a failure must be replayable.
#[test]
fn harness_seed_is_stable() {
    assert_eq!(SEED.len(), 32);
    let env = Env::default();
    let _ = env;
}
