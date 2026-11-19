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
//!
//! # Payload byte layout
//!
//! Because this mock echoes whatever it is given, the bytes an operator
//! submits must already be in the wire format the real
//! `pyth-lazer-stellar-sdk` parser expects — `polaris-market::settle` and
//! `polaris-perpetual::record_price_checkpoint` both parse the echoed bytes
//! with that SDK, not with anything this crate defines. The layout below is
//! the subset those two call sites read; every integer is **little-endian**
//! and there is no padding or alignment between fields.
//!
//! ```text
//! offset  size  field
//! 0       4     magic                 u32 = 2_479_346_549 (0x93B2_0A75)
//! 4       8     timestamp             u64, microseconds since Unix epoch
//! 12      1     channel               u8  (3 = FixedRate200ms)
//! 13      1     num_feeds             u8  (1 for a single-feed payload)
//! 14      4     feed_id               u32
//! 18      1     num_properties        u8  (3 for the properties below)
//! 19      1     property id 0         u8  = 0 (price)
//! 20      8     price                 i64, raw value: USD = price * 10^exponent
//! 28      1     property id 4         u8  = 4 (exponent)
//! 29      2     exponent              i16, base-10 power of ten
//! 31      1     property id 12        u8  = 12 (feed_update_timestamp)
//! 32      1     exists                u8  = 1 (the property is present)
//! 33      8     feed_update_timestamp u64, microseconds since Unix epoch
//! ```
//!
//! Total length: 41 bytes. `price` and `exponent` are signed and are
//! transmitted as their two's-complement bit pattern (`price as u64`,
//! `exponent as u16`); the parser reads them back as `i64`/`i16`. Units:
//! `price * 10^exponent` is the USD price, so `price = 1234_500_000`,
//! `exponent = -8` means $12.345. Both timestamps are **microseconds**;
//! the consuming contracts divide by 1_000_000 and then require the result
//! to be within `PRICE_FRESHNESS_WINDOW_SECS` (300s) of the ledger time
//! (perpetual) or of the market's expiry (market), so a payload built for
//! one ledger will not settle later.
//!
//! ## Worked example
//!
//! `feed_id = 100`, `price = 1234_500_000` ($12.345), `exponent = -8`,
//! `ts = 1_700_000_000_000_000` µs (2023-11-14T22:13:20Z):
//!
//! ```text
//! 75 0a b2 93                          magic 2_479_346_549, LE
//! 00 80 8d 5b 41 06 00 00              ts 1_700_000_000_000_000, LE
//! 03                                   channel = FixedRate200ms
//! 01                                   num_feeds = 1
//! 64 00 00 00                          feed_id = 100, LE
//! 03                                   num_properties = 3
//! 00                                   property: price
//! 00 94 95 49 00 00 00 00              price = 1234_500_000, LE
//! 04                                   property: exponent
//! f8 ff                                exponent = -8, LE (two's complement)
//! 0c                                   property: feed_update_timestamp
//! 01                                   exists = true
//! 00 80 8d 5b 41 06 00 00              feed_update_timestamp, LE
//! ```
//!
//! As one hex string:
//!
//! ```text
//! 750ab29300808d5b410600000301640000000300009495490000000004f8ff0c0100808d5b41060000
//! ```
//!
//! `test::worked_example_decodes` asserts this exact string parses back to
//! the values above, so the doc and the parser can't drift apart silently.

use soroban_sdk::{contract, contractimpl, Bytes, Env};

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
    extern crate std;

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

    /// The hex string from the module doc's worked example, decoded by hand
    /// with the documented layout — proves the doc matches the bytes.
    #[test]
    fn worked_example_decodes() {
        let env = Env::default();
        let id = env.register(MockLazer, ());
        let client = MockLazerClient::new(&env, &id);

        let hex = "750ab29300808d5b410600000301640000000300009495490000000004f8ff0c0100808d5b41060000";
        let mut raw = [0u8; 41];
        for (i, byte) in raw.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap();
        }
        let echoed = client.verify_update(&Bytes::from_slice(&env, &raw));
        assert_eq!(echoed.len(), 41);

        let mut buf = [0u8; 8];
        let at = |off: usize, len: usize| -> u64 {
            buf = [0u8; 8];
            buf[..len].copy_from_slice(&raw[off..off + len]);
            u64::from_le_bytes(buf)
        };
        assert_eq!(at(0, 4) as u32, 2_479_346_549); // magic
        assert_eq!(at(4, 8), 1_700_000_000_000_000); // timestamp (µs)
        assert_eq!(raw[12], 3); // channel
        assert_eq!(raw[13], 1); // num_feeds
        assert_eq!(at(14, 4) as u32, 100); // feed_id
        assert_eq!(raw[18], 3); // num_properties
        assert_eq!(raw[19], 0); // property: price
        assert_eq!(at(20, 8) as i64, 1_234_500_000); // price
        assert_eq!(raw[28], 4); // property: exponent
        assert_eq!(at(29, 2) as i16, -8); // exponent
        assert_eq!(raw[31], 12); // property: feed_update_timestamp
        assert_eq!(raw[32], 1); // exists
        assert_eq!(at(33, 8), 1_700_000_000_000_000); // feed_update_timestamp (µs)
    }
}
