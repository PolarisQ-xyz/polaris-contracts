extern crate std;

use soroban_sdk::xdr::ScSpecEntry;
use soroban_sdk::{Bytes, Env};
use std::{format, fs, path::Path, string::String, vec::Vec};

/// Every vendored contract, by wasm file stem, with the bytes this crate
/// actually compiled its client from.
const VENDORED: [(&str, &[u8]); 5] = [
    ("polaris_market", crate::market::WASM),
    ("polaris_perpetual", crate::perpetual::WASM),
    ("polaris_vault", crate::vault::WASM),
    ("polaris_smart_wallet", crate::smart_wallet::WASM),
    ("polaris_smart_wallet_factory", crate::smart_wallet_factory::WASM),
];

fn sha256_hex(bytes: &[u8]) -> String {
    let env = Env::default();
    let hash = env.crypto().sha256(&Bytes::from_slice(&env, bytes)).to_array();
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

/// `wasm/SHA256SUMS` is what integrators check an on-chain wasm hash
/// against, so it must describe the exact bytes vendored next to it —
/// catches a wasm replaced by hand without rerunning the sync script.
#[test]
fn vendored_wasm_matches_sha256sums() {
    let sums = include_str!("../wasm/SHA256SUMS");
    let mut listed = 0;
    for line in sums.lines().filter(|l| !l.trim().is_empty()) {
        let (hash, file) = line.split_once("  ").expect("`shasum -a 256` line format");
        let stem = file.trim().strip_suffix(".wasm").expect("a .wasm file");
        let (_, wasm) = VENDORED
            .iter()
            .find(|(name, _)| *name == stem)
            .expect("listed wasm is vendored");
        assert_eq!(sha256_hex(wasm), hash, "{file} doesn't match SHA256SUMS");
        listed += 1;
    }
    assert_eq!(listed, VENDORED.len(), "every vendored wasm is listed in SHA256SUMS");
}

fn entry_name(entry: &ScSpecEntry) -> String {
    match entry {
        ScSpecEntry::FunctionV0(f) => format!("fn {}", f.name.to_utf8_string_lossy()),
        ScSpecEntry::UdtStructV0(s) => format!("struct {}", s.name.to_utf8_string_lossy()),
        ScSpecEntry::UdtUnionV0(u) => format!("union {}", u.name.to_utf8_string_lossy()),
        ScSpecEntry::UdtEnumV0(e) => format!("enum {}", e.name.to_utf8_string_lossy()),
        ScSpecEntry::UdtErrorEnumV0(e) => format!("error enum {}", e.name.to_utf8_string_lossy()),
        ScSpecEntry::EventV0(e) => format!("event {}", e.name.to_utf8_string_lossy()),
    }
}

/// The drift guard: the interface (functions, types, errors, and their
/// doc comments) embedded in each vendored wasm must equal the one a
/// fresh build of the current source produces. Changing a contract's
/// interface without rerunning `scripts/sync-interface-wasm.sh` fails
/// here — in CI, which always builds the release wasm before testing.
///
/// Compares specs, not bytes: a different toolchain can change the code
/// bytes of an otherwise identical contract, but never its spec.
/// Skipped, loudly, for any contract whose release wasm hasn't been built
/// locally — the same build-first requirement `cargo test --workspace`
/// already has for polaris-vault (see the README) — unless
/// `POLARIS_REQUIRE_SPEC_CHECK` is set, as CI does, which makes a missing
/// build a failure instead.
#[test]
fn vendored_specs_match_current_source() {
    let release = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/wasm32v1-none/release");
    let mut drifted = Vec::new();
    for (name, vendored) in VENDORED {
        let built_path = release.join(format!("{name}.wasm"));
        let Ok(built) = fs::read(&built_path) else {
            // CI sets this so a dropped build step can't silently turn
            // the drift guard into a no-op.
            assert!(
                std::env::var_os("POLARIS_REQUIRE_SPEC_CHECK").is_none(),
                "{} not built, and POLARIS_REQUIRE_SPEC_CHECK is set",
                built_path.display()
            );
            std::eprintln!("SKIPPED spec check for {name}: {} not built", built_path.display());
            continue;
        };
        let want = soroban_spec::read::from_wasm(&built).expect("spec in built wasm");
        let have = soroban_spec::read::from_wasm(vendored).expect("spec in vendored wasm");
        if want != have {
            let changed: Vec<String> = want
                .iter()
                .filter(|e| !have.contains(e))
                .map(|e| format!("{} (source)", entry_name(e)))
                .chain(
                    have.iter()
                        .filter(|e| !want.contains(e))
                        .map(|e| format!("{} (vendored)", entry_name(e))),
                )
                .collect();
            drifted.push(format!("{name}: {}", changed.join(", ")));
        }
    }
    assert!(
        drifted.is_empty(),
        "vendored interface is stale — run scripts/sync-interface-wasm.sh. Differing entries:\n{}",
        drifted.join("\n")
    );
}

/// The vendored client and `WASM` work together end to end: deploy a
/// real market from this crate's bytes and talk to it through this
/// crate's client and error type.
#[test]
fn client_and_wasm_deploy_and_call() {
    let env = Env::default();
    let id = env.register(crate::market::WASM, ());
    let market = crate::market::Client::new(&env, &id);
    assert_eq!(
        market.try_get_market().err(),
        Some(Ok(crate::market::Error::NotInitialized))
    );
    assert!(!market.is_paused());
}
