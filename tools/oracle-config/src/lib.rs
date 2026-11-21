//! Builds the JSON `stellar contract invoke` takes for
//! `polaris-market`'s `--reflector`/`--redstone` and `polaris-perpetual`'s
//! `--price_oracle` initialize arguments, from validated inputs instead
//! of hand-edited blobs (#35).
//!
//! What's checked offline, before anything reaches the chain:
//! - every contract address is a well-formed `C...` strkey, checksum
//!   included, so a typo fails here rather than at `initialize`;
//! - each provider gets the `Asset` variant it really keys XLM under:
//!   Reflector `Other("XLM")`, RedStone `Stellar(<native XLM SAC>)` (see
//!   `polaris_ctf_math::OracleFeedConfig::asset`). Only `custom` takes a
//!   caller-chosen asset;
//! - `max_staleness_secs > 0`, `1 <= tolerance_bps <= 10_000` (the
//!   contracts store `tolerance_bps` unvalidated: `0` would reject every
//!   settle on any divergence, above `10_000` accepts any price), and a
//!   `price_oracle` `feed_id != 0` (`InvalidFeedId`).
//!
//! What can't be checked offline: that `max_staleness_secs` is at least
//! the oracle's live `resolution()`. `initialize` rejects that with
//! `InvalidReflectorConfig`/`InvalidRedstoneConfig`.

use serde_json::{json, Value};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Network {
    Testnet,
    Mainnet,
}

impl Network {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "testnet" => Ok(Self::Testnet),
            "mainnet" | "pubnet" => Ok(Self::Mainnet),
            other => Err(format!("unknown network `{other}` (expected testnet or mainnet)")),
        }
    }

    /// The native XLM Stellar Asset Contract, as printed by `stellar
    /// contract id asset --asset native` for each network's passphrase.
    pub fn native_xlm_sac(self) -> &'static str {
        match self {
            Self::Testnet => "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC",
            Self::Mainnet => "CAS3J7GYLGXMF6TDJBBYYSE3HQ6BBSMLNUQ34T6TZMYMW2EVH34XOWMA",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Reflector,
    Redstone,
    Custom,
}

impl Provider {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "reflector" => Ok(Self::Reflector),
            "redstone" => Ok(Self::Redstone),
            "custom" => Ok(Self::Custom),
            other => Err(format!(
                "unknown provider `{other}` (expected reflector, redstone or custom)"
            )),
        }
    }

    /// The contract this repo's own deployments use, where one is known
    /// (see the README's "Deploying" section). RedStone has no testnet
    /// deployment, and this build has no recorded mainnet Reflector feed.
    pub fn known_contract(self, network: Network) -> Option<&'static str> {
        match (self, network) {
            (Self::Reflector, Network::Testnet) => Some("CCYOZJCOPG34LLQQ7N24YXBM7LL62R7ONMZ3G6WZAAYPB5OYKOMJRN63"),
            (Self::Redstone, Network::Mainnet) => Some("CBMGLKUQZVSAIL5CPDDAWSUY7MAKXISHMOZEVLMBUWBMFGHRJSR4WYRF"),
            _ => None,
        }
    }

    /// The `Asset` this provider keys XLM under; `None` for `custom`.
    pub fn xlm_asset(self, network: Network) -> Option<Asset> {
        match self {
            Self::Reflector => Some(Asset::Other("XLM".into())),
            Self::Redstone => Some(Asset::Stellar(network.native_xlm_sac().into())),
            Self::Custom => None,
        }
    }
}

/// `polaris_ctf_math::sep40::Asset`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Asset {
    Stellar(String),
    Other(String),
}

impl Asset {
    /// `stellar:<C...>` or `other:<SYMBOL>`.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().split_once(':') {
            Some(("stellar", addr)) => Ok(Self::Stellar(contract_address(addr)?)),
            Some(("other", sym)) => Ok(Self::Other(symbol(sym)?)),
            _ => Err(format!(
                "asset `{s}` must be `stellar:<C... address>` or `other:<SYMBOL>`"
            )),
        }
    }

    fn to_json(&self) -> Value {
        match self {
            Self::Stellar(addr) => json!({ "Stellar": addr }),
            Self::Other(sym) => json!({ "Other": sym }),
        }
    }
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stellar(addr) => write!(f, "stellar:{addr}"),
            Self::Other(sym) => write!(f, "other:{sym}"),
        }
    }
}

/// A `C...` contract strkey, checksum verified.
pub fn contract_address(s: &str) -> Result<String, String> {
    let s = s.trim();
    match stellar_strkey::Strkey::from_string(s) {
        Ok(stellar_strkey::Strkey::Contract(_)) => Ok(s.to_string()),
        Ok(_) => Err(format!("`{s}` is a valid strkey but not a contract (C...) address")),
        Err(_) => Err(format!(
            "`{s}` is not a valid contract address (bad length, characters or checksum)"
        )),
    }
}

/// A Soroban `Symbol`: 1-32 chars of `[A-Za-z0-9_]`.
pub fn symbol(s: &str) -> Result<String, String> {
    let s = s.trim();
    if s.is_empty() || s.len() > 32 || !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(format!(
            "`{s}` is not a valid Soroban symbol (1-32 chars of A-Z, a-z, 0-9, _)"
        ));
    }
    Ok(s.to_string())
}

pub fn max_staleness_secs(s: &str) -> Result<u64, String> {
    match s.trim().parse::<u64>() {
        Ok(0) => Err("max_staleness_secs must be > 0: a zero window rejects every price".into()),
        Ok(v) => Ok(v),
        Err(_) => Err(format!(
            "max_staleness_secs `{}` is not a whole number of seconds",
            s.trim()
        )),
    }
}

pub fn tolerance_bps(s: &str) -> Result<u32, String> {
    match s.trim().parse::<u32>() {
        Ok(v @ 1..=10_000) => Ok(v),
        Ok(0) => Err("tolerance_bps must be >= 1: 0 rejects any divergence at all between oracles".into()),
        Ok(_) => Err("tolerance_bps must be <= 10000 (100%): above that the cross-check accepts any price".into()),
        Err(_) => Err(format!(
            "tolerance_bps `{}` is not a whole number of basis points",
            s.trim()
        )),
    }
}

pub fn feed_id(s: &str) -> Result<u32, String> {
    match s.trim().parse::<u32>() {
        Ok(0) => Err("feed_id must be non-zero (initialize rejects 0 as InvalidFeedId)".into()),
        Ok(v) => Ok(v),
        Err(_) => Err(format!("feed_id `{}` is not a u32", s.trim())),
    }
}

/// `polaris_ctf_math::OracleFeedConfig`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeedConfig {
    pub contract: String,
    pub asset: Asset,
    pub max_staleness_secs: u64,
    pub tolerance_bps: u32,
}

impl FeedConfig {
    pub fn to_json(&self) -> Value {
        json!({
            "contract": self.contract,
            "asset": self.asset.to_json(),
            "max_staleness_secs": self.max_staleness_secs,
            "tolerance_bps": self.tolerance_bps,
        })
    }
}

/// `polaris-perpetual`'s `PriceOracleConfig`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriceOracleConfig {
    pub lazer_contract: String,
    pub feed_id: u32,
    pub reflector: FeedConfig,
    pub redstone: FeedConfig,
}

impl PriceOracleConfig {
    /// The two `*_decimals_at_init` fields are emitted as `0`: the struct
    /// requires them, and `initialize` overwrites both with each oracle's
    /// live `decimals()` (see `PriceOracleConfig`'s doc comment there).
    pub fn to_json(&self) -> Value {
        json!({
            "lazer_contract": self.lazer_contract,
            "feed_id": self.feed_id,
            "reflector": self.reflector.to_json(),
            "reflector_decimals_at_init": 0,
            "redstone": self.redstone.to_json(),
            "redstone_decimals_at_init": 0,
        })
    }
}

#[cfg(test)]
mod test;
