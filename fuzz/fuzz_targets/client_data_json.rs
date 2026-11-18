#![no_main]

use libfuzzer_sys::fuzz_target;
use serde_json_core::de::from_slice;

/// Mirror of the smart-wallet `__check_auth` CDJ parse: a fixed 1024-byte
/// buffer, `serde_json_core` deserialization, no panics allowed.
const CDJ_BUF_LEN: usize = 1024;

#[derive(Debug, serde::Deserialize)]
struct ClientDataJson<'a> {
    #[serde(rename = "type")]
    ty: &'a str,
    challenge: &'a str,
    origin: &'a str,
}

fuzz_target!(|data: &[u8]| {
    // The contract parses attacker-controlled bytes into a fixed buffer.
    // Any input longer than the buffer must be rejected, not panic.
    if data.len() > CDJ_BUF_LEN {
        return;
    }

    let mut buf = [0u8; CDJ_BUF_LEN];
    buf[..data.len()].copy_from_slice(data);
    let input = &buf[..data.len()];

    // Primary parse with the same library the contract uses.
    let core_result: Result<ClientDataJson, _> = from_slice(input);

    // Reference parse with serde_json.
    let ref_result: Result<ClientDataJson, _> = serde_json::from_slice(input);

    match (core_result, ref_result) {
        (Ok(core), Ok(reference)) => {
            // Both parsers accepted: fields must agree.
            assert_eq!(core.ty, reference.ty);
            assert_eq!(core.challenge, reference.challenge);
            assert_eq!(core.origin, reference.origin);
        }
        (Err(_), Err(_)) => {}
        // Divergence between the contract parser and the reference parser is
        // exactly the class of bug this target exists to surface.
        (Ok(_), Err(_)) | (Err(_), Ok(_)) => {
            panic!("serde_json_core and serde_json disagree on CDJ parse");
        }
    }
});
