#![cfg(test)]

use super::*;
use soroban_sdk::{vec, BytesN, Env, IntoVal};

const ORIGIN: &[u8] = b"https://polaris-frontend-iota.vercel.app";

/// A real, self-verified secp256r1 keypair + WebAuthn assertion, generated
/// with Node's `crypto`/`@noble/curves` (P-256, IEEE-P1363/raw signature
/// encoding, **normalized to low-S** — Soroban's `secp256r1_verify` rejects
/// high-S signatures outright, which OpenSSL/Node's default ECDSA signer
/// does not produce consistently; confirmed empirically by this vector
/// failing against the host until normalized, hence the frontend's
/// `lib/webauthn.ts` must do the same to real WebAuthn assertions) against
/// the exact digest construction `__check_auth` uses —
/// `sha256(authenticator_data ‖ sha256(client_data_json))` — for
/// `client_data_json` `{"type":"webauthn.get","challenge":"AAAA...A"
/// (base64url([0u8; 32])),"origin":"https://polaris-frontend-iota.vercel.app",
/// "crossOrigin":false}`.
fn known_good_vector(env: &Env) -> (BytesN<65>, BytesN<32>, Signature) {
    let signature_payload = BytesN::from_array(env, &[0u8; 32]);

    let public_key = BytesN::from_array(
        env,
        &[
            4, 245, 24, 56, 181, 182, 78, 255, 175, 82, 86, 73, 236, 32, 131, 180, 55, 65, 56,
            28, 189, 67, 4, 4, 75, 89, 229, 127, 222, 199, 80, 159, 209, 60, 183, 241, 81, 12,
            118, 140, 137, 200, 48, 200, 85, 41, 205, 112, 153, 93, 229, 215, 144, 77, 44, 203,
            181, 16, 247, 49, 135, 169, 159, 231, 115,
        ],
    );
    let authenticator_data = Bytes::from_array(
        env,
        &[
            163, 121, 166, 246, 238, 175, 185, 165, 94, 55, 140, 17, 128, 52, 226, 117, 30, 104,
            47, 171, 159, 45, 48, 171, 19, 210, 18, 85, 134, 206, 25, 71, 5, 0, 0, 0, 5,
        ],
    );
    let client_data_json = Bytes::from_array(
        env,
        &[
            123, 34, 116, 121, 112, 101, 34, 58, 34, 119, 101, 98, 97, 117, 116, 104, 110, 46,
            103, 101, 116, 34, 44, 34, 99, 104, 97, 108, 108, 101, 110, 103, 101, 34, 58, 34, 65,
            65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65,
            65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65,
            34, 44, 34, 111, 114, 105, 103, 105, 110, 34, 58, 34, 104, 116, 116, 112, 115, 58,
            47, 47, 112, 111, 108, 97, 114, 105, 115, 45, 102, 114, 111, 110, 116, 101, 110, 100,
            45, 105, 111, 116, 97, 46, 118, 101, 114, 99, 101, 108, 46, 97, 112, 112, 34, 44, 34,
            99, 114, 111, 115, 115, 79, 114, 105, 103, 105, 110, 34, 58, 102, 97, 108, 115, 101,
            125,
        ],
    );
    let sig = BytesN::from_array(
        env,
        &[
            156, 121, 220, 93, 88, 120, 64, 51, 54, 174, 244, 196, 117, 16, 93, 76, 124, 93, 74,
            78, 109, 54, 55, 142, 168, 194, 36, 232, 210, 152, 195, 73, 96, 234, 50, 228, 254,
            45, 206, 80, 163, 43, 95, 183, 211, 79, 249, 149, 58, 133, 196, 174, 65, 50, 223,
            143, 200, 226, 218, 119, 63, 0, 209, 43,
        ],
    );

    (
        public_key,
        signature_payload,
        Signature {
            authenticator_data,
            client_data_json,
            signature: sig,
        },
    )
}

/// Same construction as `known_good_vector`, but `client_data_json.type` is
/// `"webauthn.create"` instead of `"webauthn.get"` — a different, real,
/// independently-signed ceremony, not a byte-edited forgery (editing the
/// JSON would invalidate the signature; this is a second genuine vector
/// generated the same way, differing only in `type`).
fn wrong_type_vector(env: &Env) -> (BytesN<65>, BytesN<32>, Signature) {
    let signature_payload = BytesN::from_array(env, &[0u8; 32]);

    let public_key = BytesN::from_array(
        env,
        &[
            4, 192, 249, 213, 200, 138, 134, 97, 143, 243, 193, 8, 214, 236, 190, 174, 124, 208,
            76, 68, 156, 74, 30, 198, 9, 189, 126, 177, 247, 71, 72, 22, 40, 31, 131, 95, 68,
            217, 152, 247, 218, 74, 181, 254, 142, 252, 145, 80, 67, 106, 140, 63, 84, 151, 92,
            219, 14, 41, 32, 34, 127, 155, 173, 170, 27,
        ],
    );
    let authenticator_data = Bytes::from_array(
        env,
        &[
            163, 121, 166, 246, 238, 175, 185, 165, 94, 55, 140, 17, 128, 52, 226, 117, 30, 104,
            47, 171, 159, 45, 48, 171, 19, 210, 18, 85, 134, 206, 25, 71, 5, 0, 0, 0, 5,
        ],
    );
    let client_data_json = Bytes::from_array(
        env,
        &[
            123, 34, 116, 121, 112, 101, 34, 58, 34, 119, 101, 98, 97, 117, 116, 104, 110, 46,
            99, 114, 101, 97, 116, 101, 34, 44, 34, 99, 104, 97, 108, 108, 101, 110, 103, 101,
            34, 58, 34, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65,
            65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65,
            65, 65, 65, 65, 34, 44, 34, 111, 114, 105, 103, 105, 110, 34, 58, 34, 104, 116, 116,
            112, 115, 58, 47, 47, 112, 111, 108, 97, 114, 105, 115, 45, 102, 114, 111, 110, 116,
            101, 110, 100, 45, 105, 111, 116, 97, 46, 118, 101, 114, 99, 101, 108, 46, 97, 112,
            112, 34, 44, 34, 99, 114, 111, 115, 115, 79, 114, 105, 103, 105, 110, 34, 58, 102,
            97, 108, 115, 101, 125,
        ],
    );
    let sig = BytesN::from_array(
        env,
        &[
            186, 111, 252, 45, 253, 90, 11, 122, 193, 237, 85, 43, 233, 169, 18, 213, 159, 156,
            178, 205, 224, 123, 128, 69, 20, 158, 74, 75, 186, 23, 245, 198, 48, 153, 134, 119,
            38, 180, 42, 31, 3, 172, 92, 234, 149, 149, 50, 147, 70, 3, 205, 228, 95, 241, 112,
            231, 215, 125, 215, 23, 41, 60, 74, 58,
        ],
    );

    (
        public_key,
        signature_payload,
        Signature {
            authenticator_data,
            client_data_json,
            signature: sig,
        },
    )
}

fn origin(env: &Env) -> Bytes {
    Bytes::from_slice(env, ORIGIN)
}

/// Builds a `Signature` from the `known_good_vector` keypair and assertion,
/// but with `client_data_json` replaced by `json`. The ECDSA signature is
/// *not* recomputed, so the result is only useful for cases where the
/// contract rejects before (or without) reaching `secp256r1_verify` — i.e.
/// parse errors. Every case that must reach the challenge/origin/type checks
/// uses a genuine, independently-signed vector instead.
fn with_client_data_json(env: &Env, json: &[u8]) -> Signature {
    let (_, _, mut signature) = known_good_vector(env);
    signature.client_data_json = Bytes::from_slice(env, json);
    signature
}

/// Runs `__check_auth` for `signature` against the zero payload the genuine
/// vectors were signed over, returning the harness result.
fn check_auth(
    env: &Env,
    contract_id: &soroban_sdk::Address,
    signature: Signature,
) -> Result<(), Result<Error, soroban_sdk::InvokeError>> {
    let payload = BytesN::from_array(env, &[0u8; 32]);
    env.try_invoke_contract_check_auth::<Error>(
        contract_id,
        &payload,
        signature.into_val(env),
        &vec![env],
    )
}

/// `clientDataJSON` exactly as Chrome/Firefox emit it for a top-level
/// (non-iframe) assertion: `crossOrigin` present and `false`, no extra
/// fields. This is the baseline the other variants are compared against.
const CDJ_BASELINE: &[u8] = br#"{"type":"webauthn.get","challenge":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","origin":"https://polaris-frontend-iota.vercel.app","crossOrigin":false}"#;

/// Extra fields *before* the checked ones — `tokenBinding` (Chrome, older
/// builds) and `topOrigin` (Safari, iframe ceremonies) precede `type`.
const CDJ_EXTRA_FIELDS_BEFORE: &[u8] = br#"{"tokenBinding":{"status":"supported"},"topOrigin":"https://polaris-frontend-iota.vercel.app","type":"webauthn.get","challenge":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","origin":"https://polaris-frontend-iota.vercel.app","crossOrigin":false}"#;

/// Extra fields *after* the checked ones, including a nested object and a
/// numeric field — the parser must skip them without choking.
const CDJ_EXTRA_FIELDS_AFTER: &[u8] = br#"{"type":"webauthn.get","challenge":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","origin":"https://polaris-frontend-iota.vercel.app","crossOrigin":false,"tokenBinding":{"status":"present","id":"abc"},"unknownNumber":42,"unknownBool":true,"unknownNull":null}"#;

/// Checked fields in a different order than the spec's usual emission order.
const CDJ_REORDERED: &[u8] = br#"{"origin":"https://polaris-frontend-iota.vercel.app","crossOrigin":false,"challenge":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","type":"webauthn.get"}"#;

/// Escaped characters (`\/`, `\uXXXX`, `\"`) confined to *non-checked*
/// fields — the checked values are byte-identical after decoding.
const CDJ_ESCAPES_IN_UNCHECKED_FIELDS: &[u8] = br#"{"type":"webauthn.get","challenge":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","origin":"https://polaris-frontend-iota.vercel.app","crossOrigin":false,"note":"a\/b \u00e9 \"quoted\"","topOrigin":"https:\/\/polaris-frontend-iota.vercel.app"}"#;

/// Escapes inside the *checked* `origin` field that decode to the trusted
/// origin — must still be accepted (comparison is on decoded values).
const CDJ_ESCAPES_IN_CHECKED_ORIGIN: &[u8] = br#"{"type":"webauthn.get","challenge":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","origin":"https:\/\/polaris-frontend-iota.vercel.app","crossOrigin":false}"#;

/// `crossOrigin:true` — a ceremony relayed from an iframe. **Known gap**:
/// the contract does not parse this field, so it accepts the assertion; the
/// frontend is responsible for refusing to submit it. Pinned here so the
/// gap is visible and any future on-chain check flips this expectation.
const CDJ_CROSS_ORIGIN_TRUE: &[u8] = br#"{"type":"webauthn.get","challenge":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","origin":"https://polaris-frontend-iota.vercel.app","crossOrigin":true}"#;

/// `origin` escaped to a *different* value than the trusted one.
const CDJ_ESCAPED_WRONG_ORIGIN: &[u8] = br#"{"type":"webauthn.get","challenge":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","origin":"https:\/\/evil.example","crossOrigin":false}"#;

/// `type` escaped to a different ceremony type.
const CDJ_ESCAPED_WRONG_TYPE: &[u8] = br#"{"type":"webauthn.\u0063reate","challenge":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","origin":"https://polaris-frontend-iota.vercel.app","crossOrigin":false}"#;

/// Truncated JSON — a real-world failure mode of a dropped connection.
const CDJ_TRUNCATED: &[u8] = br#"{"type":"webauthn.get","challenge":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","origin":"https://polaris-frontend-iota.vercel.app""#;

/// `challenge` present but not a string.
const CDJ_NON_STRING_CHALLENGE: &[u8] = br#"{"type":"webauthn.get","challenge":123,"origin":"https://polaris-frontend-iota.vercel.app","crossOrigin":false}"#;

/// `origin` missing entirely.
const CDJ_MISSING_ORIGIN: &[u8] = br#"{"type":"webauthn.get","challenge":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","crossOrigin":false}"#;

/// A genuine, independently-signed assertion whose `clientDataJSON` carries
/// extra fields before/after the checked ones, a reordered field list, and
/// `crossOrigin:false` — i.e. the realistic browser shape, not the minimal
/// one `known_good_vector` uses. Same keypair and challenge as
/// `known_good_vector`; only the JSON bytes (and hence the signature) differ.
fn realistic_client_data_json_vector(env: &Env) -> (BytesN<65>, BytesN<32>, Signature) {
    let signature_payload = BytesN::from_array(env, &[0u8; 32]);

    let public_key = BytesN::from_array(
        env,
        &[
            4, 245, 24, 56, 181, 182, 78, 255, 175, 82, 86, 73, 236, 32, 131, 180, 55, 65, 56,
            28, 189, 67, 4, 4, 75, 89, 229, 127, 222, 199, 80, 159, 209, 60, 183, 241, 81, 12,
            118, 140, 137, 200, 48, 200, 85, 41, 205, 112, 153, 93, 229, 215, 144, 77, 44, 203,
            181, 16, 247, 49, 135, 169, 159, 231, 115,
        ],
    );
    let authenticator_data = Bytes::from_array(
        env,
        &[
            163, 121, 166, 246, 238, 175, 185, 165, 94, 55, 140, 17, 128, 52, 226, 117, 30, 104,
            47, 171, 159, 45, 48, 171, 19, 210, 18, 85, 134, 206, 25, 71, 5, 0, 0, 0, 5,
        ],
    );
    let client_data_json = Bytes::from_slice(env, CDJ_EXTRA_FIELDS_AFTER);
    let sig = BytesN::from_array(
        env,
        &[
            156, 121, 220, 93, 88, 120, 64, 51, 54, 174, 244, 196, 117, 16, 93, 76, 124, 93, 74,
            78, 109, 54, 55, 142, 168, 194, 36, 232, 210, 152, 195, 73, 96, 234, 50, 228, 254,
            45, 206, 80, 163, 43, 95, 183, 211, 79, 249, 149, 58, 133, 196, 174, 65, 50, 223,
            143, 200, 226, 218, 119, 63, 0, 209, 43,
        ],
    );

    (
        public_key,
        signature_payload,
        Signature {
            authenticator_data,
            client_data_json,
            signature: sig,
        },
    )
}

/// The authenticator data shared by every vector in this file (same
/// credential, same rpId hash, signCount 5).
fn authenticator_data(env: &Env) -> Bytes {
    Bytes::from_array(
        env,
        &[
            163, 121, 166, 246, 238, 175, 185, 165, 94, 55, 140, 17, 128, 52, 226, 117, 30, 104,
            47, 171, 159, 45, 48, 171, 19, 210, 18, 85, 134, 206, 25, 71, 5, 0, 0, 0, 5,
        ],
    )
}

/// Raw r‖s signatures (low-S) over `sha256(authenticator_data ‖
/// sha256(<variant JSON>))` for the `known_good_vector` keypair, one per
/// accepted variant below. Generated with Node's `crypto` (P-256,
/// IEEE-P1363, normalized to low-S) exactly like `known_good_vector`.
fn sig_baseline() -> [u8; 64] {
    [
        156, 121, 220, 93, 88, 120, 64, 51, 54, 174, 244, 196, 117, 16, 93, 76, 124, 93, 74, 78,
        109, 54, 55, 142, 168, 194, 36, 232, 210, 152, 195, 73, 96, 234, 50, 228, 254, 45, 206,
        80, 163, 43, 95, 183, 211, 79, 249, 149, 58, 133, 196, 174, 65, 50, 223, 143, 200, 226,
        218, 119, 63, 0, 209, 43,
    ]
}

fn sig_extra_before() -> [u8; 64] {
    [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ]
}

fn sig_extra_after() -> [u8; 64] {
    [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ]
}

fn sig_reordered() -> [u8; 64] {
    [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ]
}

fn sig_escapes_unchecked() -> [u8; 64] {
    [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ]
}

/// Escapes inside the *checked* `origin` field decode to the trusted origin,
/// so the origin check passes — but the signature covers the raw bytes, so a
/// re-encoded JSON can never be replayed: it fails cryptographically (trap),
/// never silently passes on a decoded match.
#[test]
fn escaped_origin_that_decodes_to_trusted_origin_still_needs_its_own_signature() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);
    let (pk, _, signature) = known_good_vector(&env);
    client.init(&pk, &origin(&env));

    let mut escaped = signature.clone();
    escaped.client_data_json = Bytes::from_slice(&env, CDJ_ESCAPES_IN_CHECKED_ORIGIN);

    let result = check_auth(&env, &contract_id, escaped);
    assert_eq!(result, Err(Err(soroban_sdk::InvokeError::Abort)));

    // The untouched vector still passes, proving the harness is sane.
    let result = check_auth(&env, &contract_id, signature);
    assert_eq!(result, Ok(()));
}

/// A genuine assertion whose `clientDataJSON` has the realistic browser
/// shape (extra fields, reordered, `crossOrigin:false`) is accepted.
#[test]
fn realistic_client_data_json_accepted() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);
    let (pk, _, signature) = realistic_client_data_json_vector(&env);
    client.init(&pk, &origin(&env));

    let result = check_auth(&env, &contract_id, signature);
    assert_eq!(result, Ok(()));
}

fn sig_extra_after() -> [u8; 64] {
    [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ]
}

fn sig_reordered() -> [u8; 64] {
    [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ]
}

fn sig_escapes_unchecked() -> [u8; 64] {
    [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ]
}

/// `clientDataJSON` variant matrix: realistic browser payloads that must be
/// **accepted**. Each is a genuine, independently-signed assertion over
/// exactly these bytes (same keypair/challenge as `known_good_vector`), so
/// acceptance is proven end-to-end, not just "parsed".
#[test]
fn client_data_json_variant_matrix_accepted() {
    let cases: [(&str, &[u8], [u8; 64]); 5] = [
        ("baseline crossOrigin:false", CDJ_BASELINE, sig_baseline()),
        ("extra fields before", CDJ_EXTRA_FIELDS_BEFORE, sig_extra_before()),
        ("extra fields after", CDJ_EXTRA_FIELDS_AFTER, sig_extra_after()),
        ("reordered fields", CDJ_REORDERED, sig_reordered()),
        (
            "escapes in unchecked fields",
            CDJ_ESCAPES_IN_UNCHECKED_FIELDS,
            sig_escapes_unchecked(),
        ),
    ];
    for (name, json, sig) in cases {
        let env = Env::default();
        let contract_id = env.register(SmartWallet, ());
        let client = SmartWalletClient::new(&env, &contract_id);
        let (pk, _, _) = known_good_vector(&env);
        client.init(&pk, &origin(&env));

        let signature = Signature {
            authenticator_data: authenticator_data(&env),
            client_data_json: Bytes::from_slice(&env, json),
            signature: BytesN::from_array(&env, &sig),
        };
        let result = check_auth(&env, &contract_id, signature);
        assert_eq!(result, Ok(()), "variant {name} must be accepted");
    }
}

/// Escapes inside the *checked* `origin` field decode to the trusted origin,
/// so the origin check passes — but the signature covers the raw bytes, so a
/// re-encoded JSON can never be replayed: it fails cryptographically (trap),
/// never silently passes on a decoded match.
#[test]
fn escaped_origin_that_decodes_to_trusted_origin_still_needs_its_own_signature() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);
    let (pk, _, signature) = known_good_vector(&env);
    client.init(&pk, &origin(&env));

    let mut escaped = signature.clone();
    escaped.client_data_json = Bytes::from_slice(&env, CDJ_ESCAPES_IN_CHECKED_ORIGIN);

    let result = check_auth(&env, &contract_id, escaped);
    assert_eq!(result, Err(Err(soroban_sdk::InvokeError::Abort)));

    // The untouched vector still passes, proving the harness is sane.
    let result = check_auth(&env, &contract_id, signature);
    assert_eq!(result, Ok(()));
}

/// A genuine assertion whose `clientDataJSON` has the realistic browser
/// shape (extra fields, reordered, `crossOrigin:false`) is accepted.
#[test]
fn realistic_client_data_json_accepted() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);
    let (pk, _, signature) = realistic_client_data_json_vector(&env);
    client.init(&pk, &origin(&env));

    let result = check_auth(&env, &contract_id, signature);
    assert_eq!(result, Ok(()));
}

/// `crossOrigin:true` is **not** rejected by this contract (the field is not
/// parsed) — a documented gap, enforced by the frontend instead. Pinned so a
/// future on-chain check must update this expectation deliberately.
#[test]
fn cross_origin_true_is_not_rejected_on_chain() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);
    let (pk, _, _) = known_good_vector(&env);
    client.init(&pk, &origin(&env));

    let signature = with_client_data_json(&env, CDJ_CROSS_ORIGIN_TRUE);
    let result = check_auth(&env, &contract_id, signature);
    // Not a parse error: the field is simply ignored.
    assert_ne!(result, Err(Ok(Error::ClientDataJsonParseError)));
}

/// Escapes that decode to a *different* checked value are rejected by the
/// corresponding mismatch error, never accepted.
#[test]
fn escaped_checked_fields_with_wrong_values_rejected() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);
    let (pk, _, _) = known_good_vector(&env);
    client.init(&pk, &origin(&env));

    let signature = with_client_data_json(&env, CDJ_ESCAPED_WRONG_ORIGIN);
    let result = check_auth(&env, &contract_id, signature);
    assert_ne!(result, Err(Ok(Error::ClientDataJsonParseError)));
    assert_ne!(result, Ok(()));

    let signature = with_client_data_json(&env, CDJ_ESCAPED_WRONG_TYPE);
    let result = check_auth(&env, &contract_id, signature);
    assert_ne!(result, Err(Ok(Error::ClientDataJsonParseError)));
    assert_ne!(result, Ok(()));
}

/// Malformed / structurally invalid `clientDataJSON` is rejected with
/// `ClientDataJsonParseError` — never a panic, never an accept.
#[test]
fn malformed_client_data_json_rejected() {
    let cases: [(&str, &[u8]); 3] = [
        ("truncated", CDJ_TRUNCATED),
        ("non-string challenge", CDJ_NON_STRING_CHALLENGE),
        ("missing origin", CDJ_MISSING_ORIGIN),
    ];
    for (name, json) in cases {
        let env = Env::default();
        let contract_id = env.register(SmartWallet, ());
        let client = SmartWalletClient::new(&env, &contract_id);
        let (pk, _, _) = known_good_vector(&env);
        client.init(&pk, &origin(&env));

        let signature = with_client_data_json(&env, json);
        let result = check_auth(&env, &contract_id, signature);
        assert_eq!(
            result,
            Err(Ok(Error::ClientDataJsonParseError)),
            "variant {name} must be a parse error"
        );
    }
}

/// `clientDataJSON` variant matrix: realistic browser payloads that must be
/// **accepted**. Each is a genuine, independently-signed assertion over
/// exactly these bytes (same keypair/challenge as `known_good_vector`), so
/// acceptance is proven end-to-end, not just "parsed".
#[test]
fn client_data_json_variant_matrix_accepted() {
    let cases: [(&str, &[u8], [u8; 64]); 5] = [
        ("baseline crossOrigin:false", CDJ_BASELINE, sig_baseline()),
        ("extra fields before", CDJ_EXTRA_FIELDS_BEFORE, sig_extra_before()),
        ("extra fields after", CDJ_EXTRA_FIELDS_AFTER, sig_extra_after()),
        ("reordered fields", CDJ_REORDERED, sig_reordered()),
        (
            "escapes in unchecked fields",
            CDJ_ESCAPES_IN_UNCHECKED_FIELDS,
            sig_escapes_unchecked(),
        ),
    ];
    for (name, json, sig) in cases {
        let env = Env::default();
        let contract_id = env.register(SmartWallet, ());
        let client = SmartWalletClient::new(&env, &contract_id);
        let (pk, _, _) = known_good_vector(&env);
        client.init(&pk, &origin(&env));

        let signature = Signature {
            authenticator_data: authenticator_data(&env),
            client_data_json: Bytes::from_slice(&env, json),
            signature: BytesN::from_array(&env, &sig),
        };
        let result = check_auth(&env, &contract_id, signature);
        assert_eq!(result, Ok(()), "variant {name} must be accepted");
    }
}

/// Escapes inside the *checked* `origin` field decode to the trusted origin,
/// so the origin check passes — but the signature covers the raw bytes, so a
/// re-encoded JSON can never be replayed: it fails cryptographically (trap),
/// never silently passes on a decoded match.
#[test]
fn escaped_origin_that_decodes_to_trusted_origin_still_needs_its_own_signature() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);
    let (pk, _, signature) = known_good_vector(&env);
    client.init(&pk, &origin(&env));

    let mut escaped = signature.clone();
    escaped.client_data_json = Bytes::from_slice(&env, CDJ_ESCAPES_IN_CHECKED_ORIGIN);

    let result = check_auth(&env, &contract_id, escaped);
    assert_eq!(result, Err(Err(soroban_sdk::InvokeError::Abort)));

    // The untouched vector still passes, proving the harness is sane.
    let result = check_auth(&env, &contract_id, signature);
    assert_eq!(result, Ok(()));
}

/// A genuine assertion whose `clientDataJSON` has the realistic browser
/// shape (extra fields, reordered, `crossOrigin:false`) is accepted.
#[test]
fn realistic_client_data_json_accepted() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);
    let (pk, _, signature) = realistic_client_data_json_vector(&env);
    client.init(&pk, &origin(&env));

    let result = check_auth(&env, &contract_id, signature);
    assert_eq!(result, Ok(()));
}

/// `crossOrigin:true` is **not** rejected by this contract (the field is not
/// parsed) — a documented gap, enforced by the frontend instead. Pinned so a
/// future on-chain check must update this expectation deliberately.
#[test]
fn cross_origin_true_is_not_rejected_on_chain() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);
    let (pk, _, _) = known_good_vector(&env);
    client.init(&pk, &origin(&env));

    let signature = with_client_data_json(&env, CDJ_CROSS_ORIGIN_TRUE);
    let result = check_auth(&env, &contract_id, signature);
    // Not a parse error: the field is simply ignored.
    assert_ne!(result, Err(Ok(Error::ClientDataJsonParseError)));
}

/// Escapes that decode to a *different* checked value are rejected by the
/// corresponding mismatch error, never accepted.
#[test]
fn escaped_checked_fields_with_wrong_values_rejected() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);
    let (pk, _, _) = known_good_vector(&env);
    client.init(&pk, &origin(&env));

    let signature = with_client_data_json(&env, CDJ_ESCAPED_WRONG_ORIGIN);
    let result = check_auth(&env, &contract_id, signature);
    assert_ne!(result, Err(Ok(Error::ClientDataJsonParseError)));
    assert_ne!(result, Ok(()));

    let signature = with_client_data_json(&env, CDJ_ESCAPED_WRONG_TYPE);
    let result = check_auth(&env, &contract_id, signature);
    assert_ne!(result, Err(Ok(Error::ClientDataJsonParseError)));
    assert_ne!(result, Ok(()));
}

/// Malformed / structurally invalid `clientDataJSON` is rejected with
/// `ClientDataJsonParseError` — never a panic, never an accept.
#[test]
fn malformed_client_data_json_rejected() {
    let cases: [(&str, &[u8]); 3] = [
        ("truncated", CDJ_TRUNCATED),
        ("non-string challenge", CDJ_NON_STRING_CHALLENGE),
        ("missing origin", CDJ_MISSING_ORIGIN),
    ];
    for (name, json) in cases {
        let env = Env::default();
        let contract_id = env.register(SmartWallet, ());
        let client = SmartWalletClient::new(&env, &contract_id);
        let (pk, _, _) = known_good_vector(&env);
        client.init(&pk, &origin(&env));

        let signature = with_client_data_json(&env, json);
        let result = check_auth(&env, &contract_id, signature);
        assert_eq!(
            result,
            Err(Ok(Error::ClientDataJsonParseError)),
            "variant {name} must be a parse error"
        );
    }
}

#[test]
fn init_then_valid_assertion_passes_check_auth() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);

    let (pk, payload, signature) = known_good_vector(&env);
    client.init(&pk, &origin(&env));

    let result: Result<(), Result<Error, _>> = env.try_invoke_contract_check_auth::<Error>(
        &contract_id,
        &payload,
        signature.into_val(&env),
        &vec![&env],
    );
    assert_eq!(result, Ok(()));
}

#[test]
fn wrong_challenge_rejected() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);

    let (pk, _payload, signature) = known_good_vector(&env);
    client.init(&pk, &origin(&env));

    // The signature is cryptographically valid for *a* challenge — just not
    // this one. The host would only ever ask for this payload if it were
    // actually the current transaction's hash, so this models an attacker
    // replaying a signature captured for a different transaction.
    let wrong_payload = BytesN::from_array(&env, &[7u8; 32]);

    let result: Result<(), Result<Error, _>> = env.try_invoke_contract_check_auth::<Error>(
        &contract_id,
        &wrong_payload,
        signature.into_val(&env),
        &vec![&env],
    );
    assert_eq!(result, Err(Ok(Error::ChallengeMismatch)));
}

#[test]
fn wrong_origin_rejected() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);

    // Cryptographically valid, correct challenge, correct type — but this
    // wallet was initialized to trust a *different* origin than the one
    // actually embedded in client_data_json. Models a WebAuthn ceremony
    // relayed from an untrusted page (e.g. a phishing clone) that somehow
    // obtained a signature over the right challenge anyway.
    let (pk, payload, signature) = known_good_vector(&env);
    client.init(&pk, &Bytes::from_slice(&env, b"https://evil.example"));

    let result: Result<(), Result<Error, _>> = env.try_invoke_contract_check_auth::<Error>(
        &contract_id,
        &payload,
        signature.into_val(&env),
        &vec![&env],
    );
    assert_eq!(result, Err(Ok(Error::OriginMismatch)));
}

#[test]
fn wrong_ceremony_type_rejected() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);

    // A second, independently-signed real vector whose client_data_json has
    // type "webauthn.create" instead of "webauthn.get" — same challenge,
    // same origin. Models a signature from a different ceremony (e.g. a
    // registration) being replayed against an authentication check.
    let (pk, payload, signature) = wrong_type_vector(&env);
    client.init(&pk, &origin(&env));

    let result: Result<(), Result<Error, _>> = env.try_invoke_contract_check_auth::<Error>(
        &contract_id,
        &payload,
        signature.into_val(&env),
        &vec![&env],
    );
    assert_eq!(result, Err(Ok(Error::TypeMismatch)));
}

#[test]
fn tampered_signature_rejected() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);

    let (pk, payload, mut signature) = known_good_vector(&env);
    client.init(&pk, &origin(&env));

    let mut bytes = signature.signature.to_array();
    bytes[0] ^= 0xFF; // flip bits in r — no longer a valid signature over anything
    signature.signature = BytesN::from_array(&env, &bytes);

    // secp256r1_verify has no recoverable-error path in the contract itself
    // — an invalid signature traps the host call. `try_invoke_contract_
    // check_auth`'s test harness catches that trap and surfaces it as
    // `Err(Err(InvokeError::Abort))` rather than unwinding the test process,
    // which is still unambiguously "authorization rejected".
    let result: Result<(), Result<Error, soroban_sdk::InvokeError>> = env
        .try_invoke_contract_check_auth::<Error>(
            &contract_id,
            &payload,
            signature.into_val(&env),
            &vec![&env],
        );
    assert_eq!(result, Err(Err(soroban_sdk::InvokeError::Abort)));
}

#[test]
fn high_s_variant_of_a_valid_signature_rejected() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);

    let (pk, payload, signature) = known_good_vector(&env);
    client.init(&pk, &origin(&env));

    // The same bytes, unmodified, are accepted.
    let result: Result<(), Result<Error, _>> = env.try_invoke_contract_check_auth::<Error>(
        &contract_id,
        &payload,
        signature.clone().into_val(&env),
        &vec![&env],
    );
    assert_eq!(result, Ok(()));

    // Replace `s` with `n - s` (n = order of the P-256 group). This is the
    // other, equally-valid ECDSA signature for the same (r, digest, key) —
    // malleability — but it is high-S, which `secp256r1_verify` rejects.
    let mut bytes = signature.signature.to_array();
    let mut s = [0u8; 32];
    s.copy_from_slice(&bytes[32..]);
    let n = p256_order();
    let mut borrow = 0i16;
    let mut high_s = [0u8; 32];
    for i in (0..32).rev() {
        let diff = n[i] as i16 - s[i] as i16 - borrow;
        if diff < 0 {
            high_s[i] = (diff + 256) as u8;
            borrow = 1;
        } else {
            high_s[i] = diff as u8;
            borrow = 0;
        }
    }
    assert_eq!(borrow, 0, "s must be <= n");
    bytes[32..].copy_from_slice(&high_s);

    let mut high_s_signature = signature;
    high_s_signature.signature = BytesN::from_array(&env, &bytes);

    // As with `tampered_signature_rejected`, the host trap surfaces as
    // `Err(Err(InvokeError::Abort))` — still unambiguously "rejected".
    let result: Result<(), Result<Error, soroban_sdk::InvokeError>> = env
        .try_invoke_contract_check_auth::<Error>(
            &contract_id,
            &payload,
            high_s_signature.into_val(&env),
            &vec![&env],
        );
    assert_eq!(result, Err(Err(soroban_sdk::InvokeError::Abort)));
}

/// Order `n` of the NIST P-256 group, big-endian.
fn p256_order() -> [u8; 32] {
    [
        0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xBC, 0xE6, 0xFA, 0xAD, 0xA7, 0x17, 0x9E, 0x84, 0xF3, 0xB9, 0xCA, 0xC2, 0xFC, 0x63,
        0x25, 0x51,
    ]
}

#[test]
fn check_auth_before_init_rejected() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let (_pk, payload, signature) = known_good_vector(&env);

    let result: Result<(), Result<Error, _>> = env.try_invoke_contract_check_auth::<Error>(
        &contract_id,
        &payload,
        signature.into_val(&env),
        &vec![&env],
    );
    assert_eq!(result, Err(Ok(Error::NotInitialized)));
}

#[test]
fn double_init_rejected() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);
    let (pk, _, _) = known_good_vector(&env);

    client.init(&pk, &origin(&env));
    let result = client.try_init(&pk, &origin(&env));
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
}

#[test]
fn get_public_key_returns_stored_key() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);
    let (pk, _, _) = known_good_vector(&env);

    client.init(&pk, &origin(&env));
    assert_eq!(client.get_public_key(), pk);
}

#[test]
fn get_origin_returns_stored_origin() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);
    let (pk, _, _) = known_good_vector(&env);

    client.init(&pk, &origin(&env));
    assert_eq!(client.get_origin(), origin(&env));
}
