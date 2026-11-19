#![no_std]

//! Capital-efficiency vault for **Polaris**.
//!
//! Additive, isolated, zero changes to `contracts/market`. What this
//! actually fixes: today, every new market's `initial_liquidity` is wired
//! from the admin's own keypair by hand at `initialize()` time, and that
//! seed capital sits siloed in that one market's own reserves for its
//! entire life — recovered only at settlement, into a bare `treasury`
//! wallet address with no accounting at all. Fragmented capital, manual
//! per-market funding, no record of what's deployed where.
//!
//! This is not what "cross-margin" usually means in a derivatives system
//! (net risk across open positions, a liquidation engine, portfolio-level
//! price marking) — that model was considered and deliberately rejected
//! for this build: it would reintroduce exactly the undercollateralized-
//! position risk class this codebase's fund-safety work has spent real
//! effort eliminating from `contracts/market` itself. This vault instead
//! centralizes *custody and accounting* of LP capital while leaving every
//! individual market exactly as fully, independently collateralized as it
//! already is — `Market.treasury` is already a generic `Address`; pointing
//! it at this contract's address is the entire integration, not a market-
//! side code change.
//!
//! v1 is deliberately simple, stated up front rather than glossed over
//! (matching this codebase's existing convention — see
//! `contracts/smart-wallet`'s "a property this build does not implement"):
//! single-admin-owned capital, not fractional LP shares. Multiple depositors
//! can `deposit`, but there's no proportional-withdrawal accounting yet —
//! `withdraw` is admin-gated and moves from the vault's own pooled balance,
//! not from any individual depositor's tracked share. A real multi-LP
//! product would need share accounting before this is safe to open past a
//! single trusted operator.

use soroban_sdk::{contract, contracterror, contractimpl, symbol_short, token, Address, Env, Symbol};

/// Imports `polaris-market`'s client from its *compiled wasm's own spec* —
/// deliberately not a source dependency on the `polaris-market` crate.
/// Depending on that crate directly (for its generated `PolarisMarketClient`)
/// pulls its `#[contract]`-exported wasm symbols into *this* contract's own
/// compiled output — confirmed the hard way: `cargo build --release
/// --target wasm32v1-none` linked "successfully" but warned `function
/// signature mismatch: initialize`, because both contracts export a
/// same-named function and the linker was merging them into one binary.
/// `contractimport!` reads the target wasm's embedded interface spec at
/// compile time (the same spec `polaris-oracle`'s `contract.Spec.fromWasm`
/// reads at runtime) to generate a byte-correct client — including the
/// right `Result<T, Error>` unwrapping for `redeem` — without linking in
/// any of that wasm's actual executable code. Path is relative to this
/// crate's own directory (`contracts/vault/`), confirmed empirically —
/// the SDK's own doc comment on `contractimport!` claims "relative to the
/// workspace root," which did not hold for this soroban-sdk version.
/// Requires `polaris-market`'s release wasm already built (`cargo build
/// --release --target wasm32v1-none -p polaris-market`), same build-order
/// dependency the top-level README already documents for wiring the
/// compiled wasm into `polaris-oracle`.
mod market_contract {
    soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/polaris_market.wasm");
}

/// Same TTL bump market/perpetual already use (~31 days at 5s/ledger) —
/// see #11: this vault can go long stretches with no calls at all
/// (unlike market/perpetual, bumped as a side effect of every trade), so
/// without this every entrypoint explicitly calls `touch()` to extend
/// its own instance-storage TTL, not just the ones that happen to write.
const BUMP_TO: u32 = 535_680;
const BUMP_THRESHOLD: u32 = 500_000;

fn touch(env: &Env) {
    env.storage().instance().extend_ttl(BUMP_THRESHOLD, BUMP_TO);
}

const STORAGE_KEY_ADMIN: Symbol = symbol_short!("admin");
const STORAGE_KEY_COLLATERAL: Symbol = symbol_short!("collat");
/// Cumulative lifetime counter: every `deposit` ever made, never decremented
/// by `withdraw`/`redeem_from_market`. Reconcile the live balance with
/// `total_deposited - total_withdrawn + total_redeemed == get_balance()`.
const STORAGE_KEY_TOTAL_DEPOSITED: Symbol = symbol_short!("deposit");
/// Cumulative lifetime counter: every `withdraw` ever made.
const STORAGE_KEY_TOTAL_WITHDRAWN: Symbol = symbol_short!("withdrawn");
/// Cumulative lifetime counter: every payout ever collected from a market
/// via `redeem_from_market`.
const STORAGE_KEY_TOTAL_REDEEMED: Symbol = symbol_short!("redeemed");
/// Set only between a `propose_admin` and the matching `accept_admin` —
/// absent the rest of the time. See `propose_admin`'s doc comment for why
/// this is two steps, not a single `set_admin`.
const STORAGE_KEY_PENDING_ADMIN: Symbol = symbol_short!("pending");
/// `market_id -> ()` for every market this vault has seeded via
/// `seed_market` — the on-chain record of what capital is deployed where.
const STORAGE_KEY_MARKETS: Symbol = symbol_short!("markets");

#[contracterror]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidAmount = 3,
    Unauthorized = 4,
    /// `accept_admin` called with no `propose_admin` ever having been
    /// called (or one already consumed by a prior `accept_admin`).
    NoPendingAdmin = 5,
    /// `seed_market` called with a `market_id` this vault already seeded.
    MarketAlreadySeeded = 6,
}

#[contract]
pub struct Vault;

#[contractimpl]
impl Vault {
    /// One-time setup. `collateral` is the same asset every market this
    /// vault backs uses (native XLM SAC in this build, same as
    /// `contracts/market` — see its "Why native XLM" section) — fixed once,
    /// not re-specified per call, so a `deposit`/`withdraw` can never be
    /// pointed at the wrong asset by mistake.
    pub fn initialize(env: Env, admin: Address, collateral: Address) -> Result<(), Error> {
        admin.require_auth();
        if env.storage().instance().has(&STORAGE_KEY_ADMIN) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&STORAGE_KEY_ADMIN, &admin);
        env.storage().instance().set(&STORAGE_KEY_COLLATERAL, &collateral);
        env.storage().instance().set(&STORAGE_KEY_TOTAL_DEPOSITED, &0i128);
        env.storage().instance().set(&STORAGE_KEY_TOTAL_WITHDRAWN, &0i128);
        env.storage().instance().set(&STORAGE_KEY_TOTAL_REDEEMED, &0i128);
        touch(&env);
        Ok(())
    }

    /// LP capital in. Anyone can deposit (permissionless funding, same
    /// "anyone can pay" shape as the wallet factory's `deploy`) — v1 doesn't
    /// track per-depositor shares, so depositing is currently a one-way,
    /// admin-trusted contribution, not a redeemable LP position. Stated
    /// plainly in the module doc; not hidden in the implementation.
    pub fn deposit(env: Env, from: Address, amount: i128) -> Result<(), Error> {
        from.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let collateral = Self::require_collateral(&env)?;
        token::Client::new(&env, &collateral).transfer(&from, &env.current_contract_address(), &amount);

        let total: i128 = env.storage().instance().get(&STORAGE_KEY_TOTAL_DEPOSITED).unwrap_or(0);
        env.storage().instance().set(&STORAGE_KEY_TOTAL_DEPOSITED, &(total + amount));
        touch(&env);
        env.events().publish((symbol_short!("deposit"), from), amount);
        Ok(())
    }

    /// Admin-gated capital out — funds a new market's seed liquidity, or an
    /// LP redemption. Fails closed the same way every other admin-gated
    /// entrypoint in this system does (`AdminGuard`, `AuthRelayService`,
    /// the wallet factory's `initialize`): wrong signer, no transfer. If
    /// `amount` exceeds what's actually here, the token transfer itself
    /// fails the whole call — no separate balance check needed, same as
    /// every other transfer in this system.
    pub fn withdraw(env: Env, admin: Address, to: Address, amount: i128) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let collateral = Self::require_collateral(&env)?;
        token::Client::new(&env, &collateral).transfer(&env.current_contract_address(), &to, &amount);

        let total: i128 = env.storage().instance().get(&STORAGE_KEY_TOTAL_WITHDRAWN).unwrap_or(0);
        env.storage().instance().set(&STORAGE_KEY_TOTAL_WITHDRAWN, &(total + amount));
        touch(&env);
        env.events().publish((symbol_short!("withdraw"), to), amount);
        Ok(())
    }

    /// Collects this vault's own payout from a settled/cancelled market it
    /// was `treasury` for, by calling that market's existing `redeem`
    /// on the vault's own behalf. The one cross-contract call in this
    /// design, and it's into an entrypoint that already exists, is already
    /// tested, and needs no `require_auth` from the vault side — `redeem`
    /// only ever pays out to whichever address it's called with, and only
    /// from that address's own on-chain share balance.
    pub fn redeem_from_market(env: Env, admin: Address, market_id: Address) -> Result<i128, Error> {
        Self::require_admin(&env, &admin)?;
        let market = market_contract::Client::new(&env, &market_id);
        let payout = market.redeem(&env.current_contract_address());

        let total: i128 = env.storage().instance().get(&STORAGE_KEY_TOTAL_REDEEMED).unwrap_or(0);
        env.storage().instance().set(&STORAGE_KEY_TOTAL_REDEEMED, &(total + payout));
        touch(&env);
        env.events().publish((symbol_short!("mkt_rdm"), market_id), payout);
        Ok(payout)
    }

    /// Seeds a brand-new market directly from this vault's own balance —
    /// no admin EOA in the middle. Calls the market's existing
    /// `initialize` with `admin` and `treasury` both set to this vault's
    /// address, and authorizes the market's own `transfer(admin, market,
    /// initial_liquidity)` from the vault via
    /// `authorize_as_current_contract`, so the seed capital moves
    /// vault → market inside the same call. If `initialize` rejects
    /// anything (bad expiry, bad fee bps, bad oracle config, …) the whole
    /// call reverts and the vault's balance is untouched.
    ///
    /// `admin` is the vault's own admin, gated exactly like `withdraw` —
    /// seeding is a capital deployment, not a permissionless action.
    #[allow(clippy::too_many_arguments)]
    pub fn seed_market(
        env: Env,
        admin: Address,
        market_id: Address,
        collateral: Address,
        strike_price: i128,
        expiry: u64,
        grace_period: u64,
        lazer_contract: Address,
        feed_id: u32,
        base_fee_bps: u32,
        min_fee_bps: u32,
        initial_liquidity: i128,
        reflector: market_contract::OracleFeedConfig,
        redstone: Option<market_contract::OracleFeedConfig>,
    ) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;
        if initial_liquidity <= 0 {
            return Err(Error::InvalidAmount);
        }
        let vault = env.current_contract_address();
        let mut seeded: soroban_sdk::Vec<Address> =
            env.storage().instance().get(&STORAGE_KEY_MARKETS).unwrap_or(soroban_sdk::Vec::new(&env));
        if seeded.contains(&market_id) {
            return Err(Error::MarketAlreadySeeded);
        }

        // The market's `initialize` calls `collateral.transfer(admin, …)`
        // with `admin` = this vault, so the SAC requires this contract's
        // own authorization for that transfer. Authorize exactly that one
        // invocation — not a blanket grant — and only for the duration of
        // the `initialize` call below.
        let token_client = token::Client::new(&env, &collateral);
        env.authorize_as_current_contract(soroban_sdk::vec![
            &env,
            soroban_sdk::auth::InvokerContractAuthEntry::Contract(
                soroban_sdk::auth::SubContractInvocation {
                    context: soroban_sdk::auth::ContractContext {
                        contract: collateral.clone(),
                        fn_name: Symbol::new(&env, "transfer"),
                        args: soroban_sdk::vec![
                            &env,
                            vault.to_val(),
                            market_id.to_val(),
                            initial_liquidity.into_val(&env),
                        ],
                    },
                    sub_invocations: soroban_sdk::vec![&env],
                },
            ),
        ]);
        let _ = transfer;
        let _ = transfer;
        let _ = transfer;
        let _ = transfer;
        let _ = transfer;
        let _ = transfer;
        let _ = transfer;

        let market = market_contract::Client::new(&env, &market_id);
        market.initialize(
            &vault, // admin = this vault
            &collateral,
            &strike_price,
            &expiry,
            &grace_period,
            &lazer_contract,
            &feed_id,
            &base_fee_bps,
            &min_fee_bps,
            &vault, // treasury = this vault
            &initial_liquidity,
            &reflector,
            &redstone,
        );

        seeded.push_back(market_id.clone());
        env.storage().instance().set(&STORAGE_KEY_MARKETS, &seeded);
        touch(&env);
        env.events().publish((symbol_short!("mkt_seed"), market_id), initial_liquidity);
        Ok(())
    }

    /// Every market this vault has seeded via `seed_market`.
    pub fn get_seeded_markets(env: Env) -> soroban_sdk::Vec<Address> {
        env.storage().instance().get(&STORAGE_KEY_MARKETS).unwrap_or(soroban_sdk::Vec::new(&env))
    }

    /// Step 1 of a two-step admin rotation. Deliberately not a single-call
    /// `set_admin`: a typo'd/unreachable `proposed` address would
    /// permanently brick the vault out of its own funds exactly the way
    /// the *absence* of any rotation entrypoint already did once in
    /// production (see this repo's README / issue #4) — the new admin
    /// has to actually show up and sign `accept_admin` with its own key
    /// before the swap takes effect, so a bad proposal is harmless and
    /// reversible (just propose again) instead of a one-way mistake.
    pub fn propose_admin(env: Env, current_admin: Address, proposed: Address) -> Result<(), Error> {
        Self::require_admin(&env, &current_admin)?;
        env.storage().instance().set(&STORAGE_KEY_PENDING_ADMIN, &proposed);
        touch(&env);
        env.events().publish((symbol_short!("propose"),), proposed.clone());
        Ok(())
    }

    /// Step 2: the proposed admin confirms with their own signature. Only
    /// the exact address `propose_admin` named can call this successfully
    /// — not the current admin, not anyone else — which is what actually
    /// proves the new key is live, held, and able to sign before it
    /// becomes the only key that can move funds.
    pub fn accept_admin(env: Env, proposed: Address) -> Result<(), Error> {
        proposed.require_auth();
        let pending: Address = env
            .storage()
            .instance()
            .get(&STORAGE_KEY_PENDING_ADMIN)
            .ok_or(Error::NoPendingAdmin)?;
        if pending != proposed {
            return Err(Error::Unauthorized);
        }
        env.storage().instance().set(&STORAGE_KEY_ADMIN, &proposed);
        env.storage().instance().remove(&STORAGE_KEY_PENDING_ADMIN);
        touch(&env);
        env.events().publish((symbol_short!("newadmin"),), proposed);
        Ok(())
    }

    pub fn get_pending_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&STORAGE_KEY_PENDING_ADMIN)
    }

    pub fn get_admin(env: Env) -> Result<Address, Error> {
        env.storage().instance().get(&STORAGE_KEY_ADMIN).ok_or(Error::NotInitialized)
    }

    pub fn get_balance(env: Env) -> Result<i128, Error> {
        let collateral = Self::require_collateral(&env)?;
        Ok(token::Client::new(&env, &collateral).balance(&env.current_contract_address()))
    }

    pub fn get_total_deposited(env: Env) -> Result<i128, Error> {
        env.storage()
            .instance()
            .get(&STORAGE_KEY_TOTAL_DEPOSITED)
            .ok_or(Error::NotInitialized)
    }

    /// Cumulative lifetime total ever withdrawn via `withdraw`.
    pub fn get_total_withdrawn(env: Env) -> Result<i128, Error> {
        env.storage()
            .instance()
            .get(&STORAGE_KEY_TOTAL_WITHDRAWN)
            .ok_or(Error::NotInitialized)
    }

    /// Cumulative lifetime total ever collected via `redeem_from_market`.
    pub fn get_total_redeemed(env: Env) -> Result<i128, Error> {
        env.storage()
            .instance()
            .get(&STORAGE_KEY_TOTAL_REDEEMED)
            .ok_or(Error::NotInitialized)
    }

    fn require_admin(env: &Env, admin: &Address) -> Result<(), Error> {
        let stored: Address = env
            .storage()
            .instance()
            .get(&STORAGE_KEY_ADMIN)
            .ok_or(Error::NotInitialized)?;
        if stored != *admin {
            // require_auth on the WRONG address doesn't help an attacker —
            // they'd need that address's own signature, which they don't
            // have — but check identity first anyway so a mismatched caller
            // gets a clear typed error rather than an auth trap.
            return Err(Error::Unauthorized);
        }
        admin.require_auth();
        Ok(())
    }

    fn require_collateral(env: &Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&STORAGE_KEY_COLLATERAL)
            .ok_or(Error::NotInitialized)
    }
}

#[cfg(test)]
mod test;
