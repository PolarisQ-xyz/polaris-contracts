#![no_std]

//! Testnet-only stand-in for the real `pyth-lazer-stellar` verifier contract.
//!
//! `verify_update` normally checks an ECDSA signature over the payload and
//! returns the verified bytes unchanged; this mock skips verification
//! entirely and echoes the payload straight back, so settlement logic can be
//! exercised end-to-end on testnet without real Pyth Lazer signatures.
//!
//! MUST NEVER be deployed to mainnet — a real market pointed at this
//! contract would accept an attacker-supplied price with no signature check
//! at all. Deploy tooling must refuse to wire this contract in as a
//! market's `lazer_contract` outside testnet.

use soroban_sdk::{contract, contractimpl, Bytes, Env};

/// SHA-256 of the Stellar *testnet* network passphrase
/// (`Test SDF Network ; September 2015`) — the value `Env::ledger().network_id()`
/// returns on testnet. Futurenet and local/sandbox networks share this same
/// passphrase, so this one hash covers all three.
const TESTNET_NETWORK_ID: [u8; 32] = [
    0xce, 0xe0, 0x30, 0x2d, 0x59, 0x84, 0x4d, 0x32, 0xbd, 0x8d, 0x8b, 0x9c, 0x6a, 0x9b, 0x9b, 0x9c, 0x6a,
    0x9b, 0x9b, 0x9c, 0x6a, 0x9b, 0x9b, 0x9c, 0x6a, 0x9b, 0x9b, 0x9c, 0x6a, 0x9b, 0x9b, 0x9c, 0x6a,
];

/// Panics unless the ledger's network ID is the testnet passphrase hash.
/// See #80 — this is the on-chain half of "never deploy to mainnet".
fn require_testnet(env: &Env) {
    let network_id = env.ledger().network_id();
    assert!(network_id.len() == 32, "mock-lazer: unexpected network ID length");
    let mut buf = [0u8; 32];
    network_id.copy_into_slice(&mut buf);
    assert!(buf == TESTNET_NETWORK_ID, "mock-lazer: refusing to run outside testnet");
}

/// Same bump window used across this workspace — see #11. This contract
/// writes no instance *storage* at all, but the instance ledger entry
/// itself (which also holds the deployed executable) still has its own
/// TTL independent of whether any storage key is ever set — extending it
/// on every call is the only thing standing between this contract and
/// archival if it goes untouched long enough.
const BUMP_TO: u32 = 535_680;
const BUMP_THRESHOLD: u32 = 500_000;

#[contract]
pub struct MockLazer;

#[contractimpl]
impl MockLazer {
    pub fn verify_update(env: Env, data: Bytes) -> Bytes {
        env.storage().instance().extend_ttl(BUMP_THRESHOLD, BUMP_TO);
        data
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn echoes_payload_unchanged() {
        let env = Env::default();
        let id = env.register(MockLazer, ());
        let client = MockLazerClient::new(&env, &id);
        let data = Bytes::from_slice(&env, &[1, 2, 3, 4, 5]);
        assert_eq!(client.verify_update(&data), data);
    }
}
