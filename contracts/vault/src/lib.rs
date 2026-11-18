#![no_std]

//! Polaris vault: pooled liquidity with an admin-rotatable authority.
//!
//! Admin rotation is two-step: the current admin proposes a successor, and
//! the successor accepts. A pending proposal is bounded in time (see
//! `ADMIN_PROPOSAL_TTL_SECS`) so a stale proposal can never be accepted
//! months later, and the current admin can explicitly cancel one.

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Env, Symbol};

const STORAGE_KEY_ADMIN: Symbol = symbol_short!("admin");
const STORAGE_KEY_TOKEN: Symbol = symbol_short!("token");
const STORAGE_KEY_BALANCE: Symbol = symbol_short!("balance");
const STORAGE_KEY_TOTAL_DEPOSITED: Symbol = symbol_short!("total");
const STORAGE_KEY_PENDING_ADMIN: Symbol = symbol_short!("pending");

/// How long a pending admin proposal stays acceptable. After this window the
/// proposal is dead and `accept_admin` rejects it.
const ADMIN_PROPOSAL_TTL_SECS: u64 = 7 * 24 * 60 * 60;

const EVENT_ADMIN_PROPOSED: Symbol = symbol_short!("admin_prop");
const EVENT_ADMIN_ACCEPTED: Symbol = symbol_short!("admin_acc");
const EVENT_ADMIN_PROPOSAL_CANCELLED: Symbol = symbol_short!("admin_canc");
const EVENT_ADMIN_PROPOSAL_EXPIRED: Symbol = symbol_short!("admin_exp");

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    NoPendingAdmin = 4,
    ProposalExpired = 5,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingAdmin {
    pub proposed: Address,
    pub expires_at: u64,
}

#[contract]
pub struct Vault;

#[contractimpl]
impl Vault {
    pub fn initialize(env: Env, admin: Address, token: Address) -> Result<(), Error> {
        if env.storage().instance().has(&STORAGE_KEY_ADMIN) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&STORAGE_KEY_ADMIN, &admin);
        env.storage().instance().set(&STORAGE_KEY_TOKEN, &token);
        env.storage().instance().set(&STORAGE_KEY_BALANCE, &0i128);
        env.storage().instance().set(&STORAGE_KEY_TOTAL_DEPOSITED, &0i128);
        Ok(())
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&STORAGE_KEY_ADMIN).unwrap()
    }

    pub fn get_balance(env: Env) -> i128 {
        env.storage().instance().get(&STORAGE_KEY_BALANCE).unwrap_or(0)
    }

    pub fn get_total_deposited(env: Env) -> i128 {
        env.storage().instance().get(&STORAGE_KEY_TOTAL_DEPOSITED).unwrap_or(0)
    }

    pub fn get_pending_admin(env: Env) -> Option<Address> {
        env.storage()
            .instance()
            .get::<_, PendingAdmin>(&STORAGE_KEY_PENDING_ADMIN)
            .map(|p| p.proposed)
    }

    pub fn deposit(env: Env, from: Address, amount: i128) -> Result<(), Error> {
        if !env.storage().instance().has(&STORAGE_KEY_ADMIN) {
            return Err(Error::NotInitialized);
        }
        from.require_auth();
        let token_id: Address = env.storage().instance().get(&STORAGE_KEY_TOKEN).unwrap();
        token::Client::new(&env, &token_id).transfer(&from, &env.current_contract_address(), &amount);
        let balance = Self::get_balance(env.clone()) + amount;
        let total = Self::get_total_deposited(env.clone()) + amount;
        env.storage().instance().set(&STORAGE_KEY_BALANCE, &balance);
        env.storage().instance().set(&STORAGE_KEY_TOTAL_DEPOSITED, &total);
        Ok(())
    }

    pub fn withdraw(env: Env, admin: Address, to: Address, amount: i128) -> Result<(), Error> {
        let stored: Address = env.storage().instance().get(&STORAGE_KEY_ADMIN).unwrap();
        if admin != stored {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();
        let token_id: Address = env.storage().instance().get(&STORAGE_KEY_TOKEN).unwrap();
        token::Client::new(&env, &token_id).transfer(&env.current_contract_address(), &to, &amount);
        let balance = Self::get_balance(env.clone()) - amount;
        env.storage().instance().set(&STORAGE_KEY_BALANCE, &balance);
        Ok(())
    }

    pub fn propose_admin(env: Env, current_admin: Address, new_admin: Address) -> Result<(), Error> {
        let stored: Address = env.storage().instance().get(&STORAGE_KEY_ADMIN).unwrap();
        if current_admin != stored {
            return Err(Error::Unauthorized);
        }
        current_admin.require_auth();
        let expires_at = env.ledger().timestamp() + ADMIN_PROPOSAL_TTL_SECS;
        env.storage().instance().set(
            &STORAGE_KEY_PENDING_ADMIN,
            &PendingAdmin { proposed: new_admin.clone(), expires_at },
        );
        env.events().publish(
            (EVENT_ADMIN_PROPOSED, current_admin),
            (new_admin, expires_at),
        );
        Ok(())
    }

    pub fn cancel_admin_proposal(env: Env, current_admin: Address) -> Result<(), Error> {
        let stored: Address = env.storage().instance().get(&STORAGE_KEY_ADMIN).unwrap();
        if current_admin != stored {
            return Err(Error::Unauthorized);
        }
        current_admin.require_auth();
        let pending: PendingAdmin = env
            .storage()
            .instance()
            .get(&STORAGE_KEY_PENDING_ADMIN)
            .ok_or(Error::NoPendingAdmin)?;
        env.storage().instance().remove(&STORAGE_KEY_PENDING_ADMIN);
        env.events().publish(
            (EVENT_ADMIN_PROPOSAL_CANCELLED, current_admin),
            pending.proposed,
        );
        Ok(())
    }

    pub fn accept_admin(env: Env, new_admin: Address) -> Result<(), Error> {
        let pending: PendingAdmin = env
            .storage()
            .instance()
            .get(&STORAGE_KEY_PENDING_ADMIN)
            .ok_or(Error::NoPendingAdmin)?;
        if pending.proposed != new_admin {
            return Err(Error::Unauthorized);
        }
        new_admin.require_auth();
        if env.ledger().timestamp() > pending.expires_at {
            env.storage().instance().remove(&STORAGE_KEY_PENDING_ADMIN);
            env.events().publish(
                (EVENT_ADMIN_PROPOSAL_EXPIRED, new_admin),
                pending.expires_at,
            );
            return Err(Error::ProposalExpired);
        }
        env.storage().instance().remove(&STORAGE_KEY_PENDING_ADMIN);
        env.storage().instance().set(&STORAGE_KEY_ADMIN, &new_admin);
        env.events().publish((EVENT_ADMIN_ACCEPTED, new_admin), ());
        Ok(())
    }
}
