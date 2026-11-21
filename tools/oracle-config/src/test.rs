use super::*;
use soroban_spec_tools::Spec;

const TESTNET_REFLECTOR: &str = "CCYOZJCOPG34LLQQ7N24YXBM7LL62R7ONMZ3G6WZAAYPB5OYKOMJRN63";
const MAINNET_REDSTONE: &str = "CBMGLKUQZVSAIL5CPDDAWSUY7MAKXISHMOZEVLMBUWBMFGHRJSR4WYRF";

/// A well-formed (checksum-valid) account `G...` address.
fn account_address() -> String {
    stellar_strkey::ed25519::PublicKey([7u8; 32])
        .to_string()
        .as_str()
        .to_string()
}

fn feed(provider: Provider, network: Network, contract: &str) -> FeedConfig {
    FeedConfig {
        contract: contract.into(),
        asset: provider.xlm_asset(network).unwrap(),
        max_staleness_secs: 600,
        tolerance_bps: 150,
    }
}

/// Parses `json` as `initialize`'s `arg` parameter, exactly the way the
/// `stellar` CLI does for `stellar contract invoke -- initialize --<arg>
/// '<json>'`: the same library, the same released spec.
fn parse_as_initialize_arg(wasm: &[u8], arg: &str, json: &str) -> Result<(), String> {
    let spec = Spec::from_wasm(wasm).expect("spec");
    let init = spec.find_function("initialize").expect("initialize");
    let input = init
        .inputs
        .iter()
        .find(|i| i.name.to_utf8_string_lossy() == arg)
        .unwrap_or_else(|| panic!("initialize has no `{arg}` parameter"));
    spec.from_string(json, &input.type_)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

// ---------- the emitted JSON is what the real contracts accept ----------

#[test]
fn reflector_feed_parses_as_markets_reflector_argument() {
    let json = feed(Provider::Reflector, Network::Testnet, TESTNET_REFLECTOR)
        .to_json()
        .to_string();
    parse_as_initialize_arg(polaris_interface::market::WASM, "reflector", &json).unwrap();
}

#[test]
fn redstone_feed_parses_as_markets_optional_redstone_argument() {
    let json = feed(Provider::Redstone, Network::Mainnet, MAINNET_REDSTONE)
        .to_json()
        .to_string();
    parse_as_initialize_arg(polaris_interface::market::WASM, "redstone", &json).unwrap();
}

#[test]
fn price_oracle_parses_as_perpetuals_price_oracle_argument() {
    let cfg = PriceOracleConfig {
        lazer_contract: TESTNET_REFLECTOR.into(), // any valid C... address
        feed_id: 100,
        reflector: feed(Provider::Reflector, Network::Mainnet, TESTNET_REFLECTOR),
        redstone: feed(Provider::Redstone, Network::Mainnet, MAINNET_REDSTONE),
    };
    parse_as_initialize_arg(
        polaris_interface::perpetual::WASM,
        "price_oracle",
        &cfg.to_json().to_string(),
    )
    .unwrap();
}

/// The tests above only mean something if the parser rejects a wrong
/// shape: here, the hand-editing typo this tool exists to prevent.
#[test]
fn the_spec_parser_rejects_a_misspelled_field() {
    let json = feed(Provider::Reflector, Network::Testnet, TESTNET_REFLECTOR)
        .to_json()
        .to_string()
        .replace("max_staleness_secs", "max_stalenes_secs");
    assert!(parse_as_initialize_arg(polaris_interface::market::WASM, "reflector", &json).is_err());
}

// ---------- presets ----------

#[test]
fn each_provider_gets_the_asset_variant_it_really_keys_xlm_under() {
    assert_eq!(
        Provider::Reflector.xlm_asset(Network::Mainnet),
        Some(Asset::Other("XLM".into()))
    );
    assert_eq!(
        Provider::Redstone.xlm_asset(Network::Mainnet),
        Some(Asset::Stellar(
            "CAS3J7GYLGXMF6TDJBBYYSE3HQ6BBSMLNUQ34T6TZMYMW2EVH34XOWMA".into()
        ))
    );
    assert_eq!(
        Provider::Redstone.xlm_asset(Network::Testnet),
        Some(Asset::Stellar(
            "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC".into()
        ))
    );
    assert_eq!(Provider::Custom.xlm_asset(Network::Testnet), None);
}

#[test]
fn every_built_in_address_is_a_valid_contract_strkey() {
    for network in [Network::Testnet, Network::Mainnet] {
        contract_address(network.native_xlm_sac()).unwrap();
        for provider in [Provider::Reflector, Provider::Redstone] {
            if let Some(addr) = provider.known_contract(network) {
                contract_address(addr).unwrap();
            }
        }
    }
}

// ---------- input validation ----------

#[test]
fn a_typo_in_a_contract_address_is_caught_by_the_checksum() {
    // Last character changed: right length and alphabet, wrong checksum.
    let typo = "CCYOZJCOPG34LLQQ7N24YXBM7LL62R7ONMZ3G6WZAAYPB5OYKOMJRN64";
    assert!(contract_address(typo).is_err());
    assert!(contract_address(&TESTNET_REFLECTOR[..55]).is_err());
}

#[test]
fn an_account_address_is_not_accepted_as_a_contract() {
    let account = account_address();
    assert!(stellar_strkey::Strkey::from_string(&account).is_ok());
    let err = contract_address(&account).unwrap_err();
    assert!(err.contains("not a contract"), "{err}");
}

#[test]
fn numeric_bounds_reject_the_values_that_would_break_settlement() {
    assert!(max_staleness_secs("0").is_err());
    assert_eq!(max_staleness_secs(" 600 "), Ok(600));
    assert!(max_staleness_secs("-1").is_err());

    assert!(tolerance_bps("0").is_err());
    assert_eq!(tolerance_bps("1"), Ok(1));
    assert_eq!(tolerance_bps("10000"), Ok(10_000));
    assert!(tolerance_bps("10001").is_err());
    assert!(tolerance_bps("1.5").is_err());

    assert!(feed_id("0").is_err());
    assert_eq!(feed_id("100"), Ok(100));
}

#[test]
fn custom_assets_parse_and_validate() {
    assert_eq!(Asset::parse("other:XLM"), Ok(Asset::Other("XLM".into())));
    assert_eq!(
        Asset::parse(&format!("stellar:{TESTNET_REFLECTOR}")),
        Ok(Asset::Stellar(TESTNET_REFLECTOR.into()))
    );
    assert!(Asset::parse("other:not a symbol").is_err());
    assert!(Asset::parse("other:").is_err());
    assert!(Asset::parse(&format!("stellar:{}", account_address())).is_err());
    assert!(Asset::parse("XLM").is_err());
}

#[test]
fn feed_json_has_the_documented_readme_shape() {
    // The README deploy example's hand-written `--reflector` value.
    let readme: Value = serde_json::from_str(
        r#"{"contract":"CCYOZJCOPG34LLQQ7N24YXBM7LL62R7ONMZ3G6WZAAYPB5OYKOMJRN63","asset":{"Other":"XLM"},"max_staleness_secs":600,"tolerance_bps":150}"#,
    )
    .unwrap();
    assert_eq!(
        feed(Provider::Reflector, Network::Testnet, TESTNET_REFLECTOR).to_json(),
        readme
    );
}
