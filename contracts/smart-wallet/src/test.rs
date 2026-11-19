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

/// The upgrade invocation, authorized by the wallet's own passkey. The
/// `known_good_vector` assertion signs `signature_payload = [0u8; 32]`, so
/// the auth entry is built over exactly that payload — the same vector the
/// `__check_auth` tests use, now driving a real `upgrade` call rather than
/// being handed to `try_invoke_contract_check_auth` directly.
#[test]
fn upgrade_authorized_by_own_passkey_succeeds() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);
    let (pk, payload, signature) = known_good_vector(&env);
    client.init(&pk, &origin(&env));

    // The host asks the wallet to authorize the upgrade over `payload`; the
    // known-good assertion is valid for exactly that payload.
    let result: Result<(), Result<Error, _>> = env.try_invoke_contract_check_auth::<Error>(
        &contract_id,
        &payload,
        signature.into_val(&env),
        &vec![&env],
    );
    assert_eq!(result, Ok(()));

    // `upgrade` itself requires the wallet's own auth; mock it so the call
    // reaches `update_current_contract_wasm` (which the test host no-ops).
    env.mock_all_auths();
    let new_wasm_hash = BytesN::from_array(&env, &[9u8; 32]);
    client.upgrade(&new_wasm_hash);
    assert_eq!(client.get_public_key(), pk);
}

#[test]
fn upgrade_without_wallet_auth_rejected() {
    let env = Env::default();
    let contract_id = env.register(SmartWallet, ());
    let client = SmartWalletClient::new(&env, &contract_id);
    let (pk, _, _) = known_good_vector(&env);
    client.init(&pk, &origin(&env));

    let new_wasm_hash = BytesN::from_array(&env, &[9u8; 32]);
    let result = client.try_upgrade(&new_wasm_hash);
    assert!(result.is_err());
}
