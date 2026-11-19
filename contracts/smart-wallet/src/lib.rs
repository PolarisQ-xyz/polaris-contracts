#![no_std]

//! Polaris smart wallet — a Soroban custom account contract authorized by a
//! WebAuthn passkey (secp256r1) instead of a classic Ed25519 keypair.
//!
//! This makes a passkey-backed contract address usable anywhere a normal
//! `Address` is expected — including as the `user` argument to the market
//! contract's `buy`/`sell`/`split`/`merge`/`transfer`/`redeem` — with no
//! browser extension, no seed phrase: the browser's platform authenticator
//! (Face ID / Touch ID / Windows Hello / a hardware key) signs directly.
//!
//! ## Credit
//!
//! The `__check_auth` verification logic (challenge binding via base64url,
//! the `sha256(authenticator_data || sha256(client_data_json))` digest
//! construction, the `Signature` shape) is adapted from
//! [leighmcculloch/soroban-webauthn](https://github.com/leighmcculloch/soroban-webauthn),
//! the reference pattern the Stellar ecosystem uses for this exact problem.
//! That repo's own README calls it "demo material only... not audited" —
//! the same caveat applies here. This is a from-scratch educational build,
//! not something to hold real value without an independent security review.
//!
//! ## How authorization works
//!
//! Soroban's account-abstraction model lets any contract stand in for a
//! signer by implementing `CustomAccountInterface::__check_auth`. When a
//! transaction includes an authorization entry for this contract's address,
//! the host calls `__check_auth` with:
//! - `signature_payload`: a 32-byte hash the host computed *itself* from the
//!   transaction's authorized invocation tree — this is what must actually
//!   have been signed. A contract can't be tricked into accepting a
//!   signature over the wrong payload because it never chooses this value.
//! - `signature`: whatever custom data the caller supplied — here, the raw
//!   WebAuthn assertion (`authenticator_data`, `client_data_json`, and the
//!   64-byte raw-r‖s ECDSA `signature`).
//!
//! WebAuthn doesn't sign `signature_payload` directly — it signs
//! `authenticator_data ‖ sha256(client_data_json)`, and `client_data_json`
//! contains a `challenge` field that the *browser* base64url-encodes from
//! whatever bytes it was asked to sign. So verification has three parts:
//! 1. Cryptographic: does `signature` verify against the wallet's stored
//!    public key for `sha256(authenticator_data ‖ sha256(client_data_json))`?
//! 2. Challenge binding: does `client_data_json.challenge` actually equal
//!    `base64url(signature_payload)` — i.e. did the passkey sign *this*
//!    transaction and not some other payload?
//! 3. Relying-party binding (`type`/`origin`, WebAuthn spec §7.2): does
//!    `client_data_json.type` equal `"webauthn.get"`, and `.origin` equal
//!    the one origin this wallet was initialized to trust? A signature
//!    cryptographically covers *whatever* ceremony produced it, for *any*
//!    origin that shares this wallet's `rpId` scope — (1) and (2) alone
//!    prove "the right key signed the right challenge," not "this ceremony
//!    was actually performed in a context the wallet owner meant to
//!    authorize a transaction from." A WebAuthn ceremony silently relayed
//!    from an untrusted page (a phishing clone, a different origin under
//!    the same `rpId` registrable domain) would pass (1) and (2) with no
//!    way to tell it apart from the real thing without this third check.
//! Skipping any of the three breaks the scheme: (1) alone accepts a valid
//! signature over an unrelated challenge; (2) alone has nothing to verify
//! against; (1)+(2) without (3) can't distinguish *where* the signing
//! ceremony actually happened.
//!
//! ## What `__check_auth` still does not constrain: *what* was authorized
//!
//! All three checks above prove the owner's passkey approved *this*
//! transaction's `signature_payload`. They say nothing about the contents of
//! that payload: `_auth_contexts` is accepted and ignored, so a single valid
//! assertion authorizes every invocation in the tree the host built — any
//! contract, any function, any amount. A malicious frontend or embedder that
//! gets the owner to approve one Face ID prompt can therefore have the wallet
//! call anything the wallet's address is allowed to call. An optional
//! allowlist / spend-cap policy enforced by iterating `auth_contexts` is
//! designed in `docs/issues/92-wallet-policy.md`; it is not implemented here.
//!
//! ## `authenticator_data.rpIdHash` (WebAuthn §7.2 step 13) — checked
//!
//! The first 32 bytes of `authenticator_data` are `rpIdHash =
//! sha256(rpId)`, the relying party the authenticator believes it is
//! signing for. §7.2 requires the verifier to compare it against
//! `sha256(expected rpId)` and reject on mismatch. This build does check
//! it: `init` pins `sha256(rp_id)` alongside the origin (the factory
//! passes it, derived from the same pinned origin), and `__check_auth`
//! rejects any assertion whose `rpIdHash` differs with `RpIdMismatch`.
//! Without it, an assertion produced for a *different* relying party that
//! happens to share this wallet's key would be accepted as long as its
//! `client_data_json` origin matched — the origin check alone is a
//! client-supplied string, while `rpIdHash` is covered by the signature
//! and set by the authenticator itself.
//!
//! ## A property this build does not implement
//!
//! Real WebAuthn deployments must track and reject replayed authenticator
//! `signCount` values (clone detection) and typically support multiple
//! registered signers per wallet with revocation. This build stores exactly
//! one public key and one trusted origin, set once at `init`, with no
//! rotation — enough to demonstrate the passkey-as-Soroban-signer
//! mechanism, not a production account-recovery story. A future frontend
//! domain change (this project has already moved domains more than once)
//! means a new factory deployment pinned to the new origin, same pattern
//! already used for this project's other no-rotation gaps (see this
//! repo's vault admin-rotation issue) — existing wallets keep running
//! their original wasm, bound to their original origin, unaffected.

use soroban_sdk::{
    auth::{Context, CustomAccountInterface},
    contract, contracterror, contractimpl, crypto::Hash, symbol_short, Bytes, BytesN, Env, Symbol,
    Vec,
};

mod base64_url;

#[cfg(test)]
mod test;

const STORAGE_KEY_PK: Symbol = symbol_short!("pk");
const STORAGE_KEY_ORIGIN: Symbol = symbol_short!("origin");
const STORAGE_KEY_RP_ID_HASH: Symbol = symbol_short!("rpidhash");
/// Same bump window `contracts/market`/`contracts/perpetual` use — see
/// #11: unlike those two (bumped as a side effect of every trade), this
/// contract is only ever touched by `init` (once) and `__check_auth`
/// (only when its owner actually trades), so a wallet that goes dormant
/// for a long stretch needs an explicit extend, not an incidental one.
const BUMP_TO: u32 = 535_680;
const BUMP_THRESHOLD: u32 = 500_000;

fn touch(env: &Env) {
    env.storage().instance().extend_ttl(BUMP_THRESHOLD, BUMP_TO);
}

#[contracterror]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    ChallengeMismatch = 3,
    ClientDataJsonParseError = 4,
    OriginMismatch = 5,
    TypeMismatch = 6,
    RpIdMismatch = 7,
    AuthenticatorDataTooShort = 8,
    UserNotVerified = 9,
}

#[contract]
pub struct SmartWallet;

#[contractimpl]
impl SmartWallet {
    /// One-time init with the wallet's secp256r1 public key (65-byte
    /// uncompressed SEC1 point, as returned by
    /// `crypto.subtle.exportKey('raw', ...)` on the passkey's public key)
    /// and the WebAuthn relying-party origin this wallet will ever accept
    /// a signature from (e.g. `b"https://polaris-frontend-iota.vercel.app"`).
    ///
    /// `origin` is fixed for the wallet's lifetime, same as `public_key` —
    /// see `__check_auth`'s doc comment for why this can't be a
    /// `deploy()`-time caller parameter instead.
    ///
    /// `rp_id_hash` is `sha256(rp_id)` — the value WebAuthn puts in the
    /// first 32 bytes of `authenticator_data` — pinned here so
    /// `__check_auth` can enforce WebAuthn §7.2's `rpIdHash` check. The
    /// factory derives it from the same pinned origin it passes as
    /// `origin`, so the two can never disagree.
    pub fn init(
        env: Env,
        public_key: BytesN<65>,
        origin: Bytes,
        rp_id_hash: BytesN<32>,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&STORAGE_KEY_PK) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&STORAGE_KEY_PK, &public_key);
        env.storage().instance().set(&STORAGE_KEY_ORIGIN, &origin);
        env.storage()
            .instance()
            .set(&STORAGE_KEY_RP_ID_HASH, &rp_id_hash);
        touch(&env);
        Ok(())
    }

    pub fn get_public_key(env: Env) -> Result<BytesN<65>, Error> {
        env.storage()
            .instance()
            .get(&STORAGE_KEY_PK)
            .ok_or(Error::NotInitialized)
    }

    pub fn get_origin(env: Env) -> Result<Bytes, Error> {
        env.storage()
            .instance()
            .get(&STORAGE_KEY_ORIGIN)
            .ok_or(Error::NotInitialized)
    }

    pub fn get_rp_id_hash(env: Env) -> Result<BytesN<32>, Error> {
        env.storage()
            .instance()
            .get(&STORAGE_KEY_RP_ID_HASH)
            .ok_or(Error::NotInitialized)
    }
}

/// The WebAuthn assertion, passed by the caller as this account's
/// authorization "signature". `signature` is the raw 64-byte r‖s ECDSA
/// signature. Browsers return DER by default, and DER-decoded (r, s) is not
/// guaranteed low-S — Soroban's `secp256r1_verify` traps on a high-S
/// signature (confirmed empirically in `test.rs`: an otherwise-valid
/// signature generated without normalizing was rejected by the host until
/// `s` was replaced with `n - s`). The frontend must convert DER to raw r‖s
/// *and* normalize to low-S before submitting; see
/// `polaris-frontend`'s `lib/webauthn.ts`.
#[derive(Clone)]
#[soroban_sdk::contracttype]
pub struct Signature {
    pub authenticator_data: Bytes,
    pub client_data_json: Bytes,
    pub signature: BytesN<64>,
}

#[derive(serde::Deserialize)]
struct ClientDataJson<'a> {
    #[serde(rename = "type")]
    type_: &'a str,
    challenge: &'a str,
    origin: &'a str,
}

#[contractimpl]
impl CustomAccountInterface for SmartWallet {
    type Error = Error;
    type Signature = Signature;

    #[allow(non_snake_case)]
    fn __check_auth(
        env: Env,
        signature_payload: Hash<32>,
        signature: Signature,
        // Deliberately unused: this wallet authorizes *who* signed, not
        // *what* was signed. See the module doc and
        // `docs/issues/92-wallet-policy.md` for the optional policy that
        // would iterate these contexts.
        _auth_contexts: Vec<Context>,
    ) -> Result<(), Error> {
        let public_key = env
            .storage()
            .instance()
            .get(&STORAGE_KEY_PK)
            .ok_or(Error::NotInitialized)?;

        // What WebAuthn actually signs: authenticator_data ‖ sha256(client_data_json).
        let mut signed = Bytes::new(&env);
        signed.append(&signature.authenticator_data);
        signed.append(&env.crypto().sha256(&signature.client_data_json).to_bytes().into());
        let digest = env.crypto().sha256(&signed);

        // Traps if invalid — there is nothing more specific to branch on,
        // and a trap is exactly "reject this authorization", which is the
        // only sane response to a bad signature here.
        env.crypto()
            .secp256r1_verify(&public_key, &digest, &signature.signature);

        // Cryptographic validity alone isn't enough: it proves *some*
        // challenge was signed by this key, not that *this transaction*
        // was. Bind them by checking the browser-supplied challenge field
        // equals what the host computed for this specific invocation tree.
        let client_data_json = signature.client_data_json.to_buffer::<1024>();
        let (client_data, _): (ClientDataJson, _) =
            serde_json_core::de::from_slice(client_data_json.as_slice())
                .map_err(|_| Error::ClientDataJsonParseError)?;

        let mut expected_challenge = [0u8; 43]; // 32 bytes -> 43 base64url chars, unpadded
        base64_url::encode(&mut expected_challenge, &signature_payload.to_array());

        if client_data.challenge.as_bytes() != expected_challenge {
            return Err(Error::ChallengeMismatch);
        }

        // Relying-party binding — see the module doc's "three parts" for
        // why cryptographic validity plus challenge binding alone isn't
        // enough. `type` must be exactly the assertion ceremony type (not
        // e.g. `webauthn.create`, which signs a different structure but
        // could otherwise collide); `origin` must match what this specific
        // wallet was initialized to trust.
        if client_data.type_ != "webauthn.get" {
            return Err(Error::TypeMismatch);
        }

        let expected_origin: Bytes = env
            .storage()
            .instance()
            .get(&STORAGE_KEY_ORIGIN)
            .ok_or(Error::NotInitialized)?;
        if Bytes::from_slice(&env, client_data.origin.as_bytes()) != expected_origin {
            return Err(Error::OriginMismatch);
        }

        // WebAuthn §7.2 step 13: the first 32 bytes of `authenticator_data`
        // are `sha256(rpId)` — set by the authenticator, covered by the
        // signature, and therefore not something a relaying page can forge.
        // Must equal the `sha256(rp_id)` pinned at `init`.
        let expected_rp_id_hash: BytesN<32> = env
            .storage()
            .instance()
            .get(&STORAGE_KEY_RP_ID_HASH)
            .ok_or(Error::NotInitialized)?;
        if signature.authenticator_data.len() < 32 {
            return Err(Error::RpIdMismatch);
        }
        let mut rp_id_hash = [0u8; 32];
        rp_id_hash.copy_from_slice(
            signature
                .authenticator_data
                .slice(0..32)
                .to_buffer::<32>()
                .as_slice(),
        );
        if BytesN::from_array(&env, &rp_id_hash) != expected_rp_id_hash {
            return Err(Error::RpIdMismatch);
        }

        // WebAuthn §6.1: `authenticator_data` is
        //   rpIdHash(32) ‖ flags(1) ‖ signCount(4) ‖ ...
        // so the flags byte is at offset 32 and the fixed prefix is 37
        // bytes. Anything shorter can't carry flags at all — reject with a
        // typed error rather than slicing out of bounds (which would trap).
        if signature.authenticator_data.len() < 37 {
            return Err(Error::AuthenticatorDataTooShort);
        }
        let flags = signature.authenticator_data.get(32).unwrap_or(0);
        const FLAG_UP: u8 = 0x01; // user present
        const FLAG_UV: u8 = 0x04; // user verified
        // UP must be set: the user was physically present at the
        // authenticator. UV must be set too — this wallet's whole premise is
        // a Face ID / Touch ID / Windows Hello passkey, i.e. a *verified*
        // user; accepting a bare presence check would silently downgrade
        // that promise.
        if flags & FLAG_UP == 0 || flags & FLAG_UV == 0 {
            return Err(Error::UserNotVerified);
        }

        touch(&env);
        Ok(())
    }
}
