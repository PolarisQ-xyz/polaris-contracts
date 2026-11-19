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
//!
//! ## Accepted `clientDataJSON` shapes (and deliberate rejections)
//!
//! `client_data_json` is parsed with `serde-json-core` into a struct holding
//! only `type`, `challenge` and `origin`; every other field is ignored. The
//! test matrix in `test.rs` pins the resulting behaviour:
//! - **Accepted**: extra fields before/after the checked ones (`tokenBinding`,
//!   `topOrigin`, `crossOrigin`, …), any field order, `crossOrigin:false`,
//!   and escaped characters (`\uXXXX`, `\/`, `\"`) inside *non-checked*
//!   fields. The signature covers the exact bytes, so these are genuine
//!   vectors, not byte-edited ones.
//! - **Deliberately rejected**: malformed JSON, missing `type`/`challenge`/
//!   `origin`, and non-string values for those keys — all
//!   `ClientDataJsonParseError`. Escapes inside a *checked* field are
//!   compared after decoding, so `"https:\/\/…"` still matches the stored
//!   origin; an escape that decodes to a different value is an
//!   `OriginMismatch`/`ChallengeMismatch`/`TypeMismatch`, never a silent
//!   pass.
//! - **Known gap, not enforced here**: `crossOrigin:true` is *not* rejected
//!   by this contract — the field is not parsed, so a ceremony relayed from
//!   an iframe would pass. The frontend must refuse to submit such
//!   assertions; adding the check on-chain is tracked separately.

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
    pub fn init(env: Env, public_key: BytesN<65>, origin: Bytes) -> Result<(), Error> {
        if env.storage().instance().has(&STORAGE_KEY_PK) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&STORAGE_KEY_PK, &public_key);
        env.storage().instance().set(&STORAGE_KEY_ORIGIN, &origin);
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
}

/// The WebAuthn assertion, passed by the caller as this account's
/// authorization "signature". `signature` is the raw 64-byte r‖s ECDSA
/// signature. Browsers return DER by default, and DER-decoded (r, s) is not
/// guaranteed low-S — Soroban's `secp256r1_verify` traps on a high-S
/// signature (regression-tested by
/// `test.rs::high_s_variant_of_a_valid_signature_rejected`, which shows the
/// same bytes passing with low-S and being rejected once `s` is replaced
/// with `n - s`). The frontend must convert DER to raw r‖s
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

        touch(&env);
        Ok(())
    }
}
