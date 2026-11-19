#![no_std]

//! Deploys and initializes a `polaris-smart-wallet` instance in one
//! transaction.
//!
//! Soroban doesn't support atomically deploying an arbitrary contract *and*
//! calling an arbitrary constructor from off-chain in one step for wasm
//! deployed this way — but a contract itself can deploy a child contract
//! and immediately invoke it, since `deploy` + `invoke_contract` inside a
//! single contract call is one atomic host invocation. So the standard
//! pattern (used by the reference this is adapted from) is a tiny factory
//! that does both.
//!
//! The deploy salt is `sha256(public_key)`, which makes the wallet's
//! contract address a deterministic function of the passkey's public key:
//! the same passkey always resolves to the same address, on any frontend
//! that points at this same factory instance — a genuinely portable
//! identity, not just an implementation detail. `resolve` computes that
//! address via the exact same on-chain formula `deploy` uses, without
//! deploying anything — a caller (this system's own backend, or any third
//! party embedding a market) can check whether a wallet already exists for
//! a given passkey before prompting a "create wallet" flow. `deploy` itself
//! is idempotent: if a wallet already exists at that key's address, it
//! probes the existing contract (via `get_public_key`) and returns that
//! address unchanged instead of trapping in `deploy_v2` — so a second call
//! for the same key is a safe no-op, never a duplicate or a hijack.
//!
//! Deliberately *not* reimplemented off-chain (in the backend or frontend):
//! Soroban's exact deployer-address-plus-salt hash isn't something worth
//! hand-rolling and hoping matches the host's real derivation when the
//! genuine on-chain computation is one cheap, free simulated call away.
//!
//! ## The wasm this factory deploys is pinned, not caller-chosen
//!
//! `wasm_hash` used to be a `deploy()` parameter — any caller could deploy
//! *any* already-uploaded wasm at any public key's deterministic address,
//! unauthenticated. Confirmed exploitable, not just theoretical: since the
//! deployed address is a pure function of (this factory, sha256(public_key))
//! — the same formula `resolve()` exposes as a public read for anyone to
//! compute in advance — an attacker who learns a victim's public key before
//! the legitimate deploy transaction lands (e.g. watching it sit in the
//! network's public mempool) could race a `deploy(victim_pk,
//! attacker_wasm_hash)` ahead of it. `deploy_v2` at a given (deployer, salt)
//! only ever succeeds once, so whichever call lands first *permanently*
//! owns that address — a malicious contract designed to look like a
//! smart-wallet while authorizing whatever the attacker wants would then
//! be able to take anything later sent to what the whole system believes
//! is the victim's wallet. See `bug_deploy_used_to_let_anyone_pick_the_
//! wasm_and_hijack_a_victims_address` in `test.rs` for the reproduction.
//!
//! Fixed by pinning the wasm hash once, at factory setup
//! (`__constructor(admin, wasm_hash, origin)`, admin-authenticated, runs
//! atomically with the deploy — see #95), and
//! having `deploy` always use that stored hash. `deploy` itself stays
//! deliberately unauthenticated — "anyone can pay to deploy anyone's
//! wallet" is the intended, safe design the backend already relies on to
//! onboard users with zero XLM — but now that's only ever safe because the
//! code being deployed is always this factory's own known-good wallet
//! implementation, never a caller's choice.

use soroban_sdk::{contract, contracterror, contractimpl, symbol_short, vec, Address, Bytes, BytesN, Env, Symbol};

const STORAGE_KEY_WASM_HASH: Symbol = symbol_short!("wasm");
const STORAGE_KEY_ORIGIN: Symbol = symbol_short!("origin");
const STORAGE_KEY_ADMIN: Symbol = symbol_short!("admin");
/// Same bump window used across this workspace — see #11. The constructor
/// only ever runs once per factory instance, so without this the
/// factory's own storage TTL has nothing else to keep it alive between
/// then and whenever `deploy` is next called — which, on a low-traffic
/// deployment, could be a long time.
const BUMP_TO: u32 = 535_680;
const BUMP_THRESHOLD: u32 = 500_000;

fn touch(env: &Env) {
    env.storage().instance().extend_ttl(BUMP_THRESHOLD, BUMP_TO);
}

#[contracterror]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum Error {
    /// Retained for ABI stability; the constructor (#95) can no longer
    /// report it — a second deploy at the same address fails in the host.
    AlreadyInitialized = 1,
    NotInitialized = 2,
}

#[contract]
pub struct SmartWalletFactory;

#[contractimpl]
impl SmartWalletFactory {
    /// One-time setup pinning which wallet code this factory will ever
    /// deploy, and which WebAuthn relying-party `origin` every wallet it
    /// deploys will be bound to (see `polaris-smart-wallet`'s `__check_auth`
    /// doc comment for why this can't safely be a `deploy()`-time caller
    /// parameter instead: `deploy` is deliberately unauthenticated, and a
    /// caller-chosen origin would let anyone front-run a victim's own
    /// `deploy` call and permanently brick their wallet with a wrong
    /// origin before they ever use it — the exact same shape of bug this
    /// factory already fixed once for `wasm_hash`, see this file's own
    /// module doc, applied proactively here instead of waiting to find it
    /// the same way.
    ///
    /// `admin` is whoever controls this factory's configuration — not a
    /// role any deployed wallet answers to, just the authority to set this
    /// once. It is authenticated here and then stored, so the factory keeps
    /// a lasting, readable record of that authority (`get_admin`) instead of
    /// discarding it after the auth check (#96).
    ///
    /// A `__constructor` (#95), not a separate `initialize` entrypoint:
    /// `deploy` + `initialize` as two transactions left a window where the
    /// factory existed unconfigured, and anyone could race the legitimate
    /// setup. The constructor runs atomically inside the deploy invocation,
    /// so the factory is never observable in an unconfigured state.
    pub fn __constructor(env: Env, admin: Address, wasm_hash: BytesN<32>, origin: Bytes) {
        admin.require_auth();
        env.storage().instance().set(&STORAGE_KEY_WASM_HASH, &wasm_hash);
        env.storage().instance().set(&STORAGE_KEY_ORIGIN, &origin);
        env.storage().instance().set(&STORAGE_KEY_ADMIN, &admin);
        touch(&env);
    }

    /// The admin this factory was constructed with — the authority that
    /// pinned the wasm hash and origin. A pure read, safe to call
    /// speculatively.
    pub fn get_admin(env: Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&STORAGE_KEY_ADMIN)
            .ok_or(Error::NotInitialized)
    }

    /// Deploys (and atomically initializes) a smart-wallet for
    /// `public_key`, always running this factory's own pinned wasm and
    /// bound to this factory's own pinned origin — see the module doc for
    /// why neither is a parameter here. Deliberately unauthenticated:
    /// anyone can pay to deploy anyone's wallet, which is what lets this
    /// system's backend onboard a user who holds zero XLM.
    ///
    /// Idempotent: if a contract already lives at `public_key`'s
    /// deterministic address, this returns that address without deploying
    /// or re-initializing anything. The probe is a `get_public_key` call on
    /// the existing contract, so an address occupied by something that
    /// isn't an initialized smart wallet still traps rather than being
    /// silently handed back.
    pub fn deploy(env: Env, public_key: BytesN<65>) -> Result<Address, Error> {
        let wasm_hash: BytesN<32> = env
            .storage()
            .instance()
            .get(&STORAGE_KEY_WASM_HASH)
            .ok_or(Error::NotInitialized)?;
        let origin: Bytes = env
            .storage()
            .instance()
            .get(&STORAGE_KEY_ORIGIN)
            .ok_or(Error::NotInitialized)?;
        let salt = env.crypto().sha256(&public_key.clone().into());
        let address = env
            .deployer()
            .with_current_contract(salt.clone())
            .deployed_address();
        let existing: Result<Result<BytesN<65>, _>, _> =
            env.try_invoke_contract(&address, &symbol_short!("get_pk"), vec![&env]);
        if let Ok(Ok(existing_pk)) = existing {
            if existing_pk == public_key {
                touch(&env);
                return Ok(address);
            }
        }
        let address = env
            .deployer()
            .with_current_contract(salt)
            .deploy_v2(wasm_hash, ());
        let _: () = env.invoke_contract(
            &address,
            &symbol_short!("init"),
            vec![&env, public_key.to_val(), origin.to_val()],
        );
        touch(&env);
        Ok(address)
    }

    /// The address `deploy(public_key)` would produce, computed without
    /// deploying anything — a pure read, safe to call speculatively.
    pub fn resolve(env: Env, public_key: BytesN<65>) -> Address {
        let salt = env.crypto().sha256(&public_key.into());
        env.deployer().with_current_contract(salt).deployed_address()
    }
}

#[cfg(test)]
mod test;
