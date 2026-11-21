//! `polaris-oracle-config`: prints a validated `OracleFeedConfig` or
//! `PriceOracleConfig` as one line of JSON on stdout, ready to pass to
//! `stellar contract invoke -- initialize --reflector "$(...)"`. Any value
//! not given as a flag is prompted for when stdin is a terminal (invalid
//! answers are re-asked); otherwise it's an error. Prompts, notes and
//! errors go to stderr, so stdout is only ever the JSON. See `lib.rs` for
//! what is validated.

use polaris_oracle_config::{
    contract_address, feed_id, max_staleness_secs, tolerance_bps, Asset, FeedConfig, Network, PriceOracleConfig,
    Provider,
};
use std::collections::BTreeMap;
use std::io::{self, BufRead, IsTerminal, Write};
use std::process::ExitCode;

const USAGE: &str = "\
Usage:
  polaris-oracle-config feed --network <testnet|mainnet> --provider <reflector|redstone|custom> \\
      [--contract <C...>] [--asset <stellar:C...|other:SYMBOL>] \\
      --max-staleness-secs <N> --tolerance-bps <N>
  polaris-oracle-config price-oracle --network <testnet|mainnet> \\
      --lazer-contract <C...> --feed-id <N> \\
      [--reflector-contract <C...>] --reflector-max-staleness-secs <N> --reflector-tolerance-bps <N> \\
      [--redstone-contract <C...>] --redstone-max-staleness-secs <N> --redstone-tolerance-bps <N>

`feed` builds one OracleFeedConfig: polaris-market's --reflector or --redstone.
`price-oracle` builds polaris-perpetual's --price_oracle (Reflector + RedStone legs).

--contract defaults to the provider's known deployment where there is one
(Reflector on testnet, RedStone on mainnet). --asset is only accepted with
--provider custom: reflector and redstone always use the asset variant each
really keys XLM under.

Example:
  stellar contract invoke --id <MARKET> --network testnet -- initialize ... \\
    --reflector \"$(polaris-oracle-config feed --network testnet --provider reflector \\
                    --max-staleness-secs 600 --tolerance-bps 150)\"";

struct Inputs {
    flags: BTreeMap<String, String>,
    interactive: bool,
}

impl Inputs {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut flags = BTreeMap::new();
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            let key = arg
                .strip_prefix("--")
                .ok_or_else(|| format!("unexpected argument `{arg}`"))?;
            let (key, value) = match key.split_once('=') {
                Some((k, v)) => (k.to_string(), v.to_string()),
                None => (
                    key.to_string(),
                    it.next().ok_or_else(|| format!("--{key} needs a value"))?.clone(),
                ),
            };
            if flags.insert(key.clone(), value).is_some() {
                return Err(format!("--{key} given twice"));
            }
        }
        Ok(Self {
            flags,
            interactive: io::stdin().is_terminal(),
        })
    }

    /// The value of `--key`, or a prompted answer. `default` is offered
    /// at the prompt (and used for an empty answer) but never applied
    /// silently to a missing flag outside a terminal.
    fn get<T>(
        &mut self,
        key: &str,
        prompt: &str,
        default: Option<&str>,
        parse: impl Fn(&str) -> Result<T, String>,
    ) -> Result<T, String> {
        if let Some(value) = self.flags.remove(key) {
            return parse(&value).map_err(|e| format!("--{key}: {e}"));
        }
        if !self.interactive {
            return Err(format!("missing --{key} (no terminal to prompt on)"));
        }
        let stdin = io::stdin();
        loop {
            match default {
                Some(d) => eprint!("{prompt} [{d}]: "),
                None => eprint!("{prompt}: "),
            }
            io::stderr().flush().ok();
            let mut line = String::new();
            if stdin.lock().read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                return Err(format!("missing --{key} (stdin closed)"));
            }
            let answer = match (line.trim(), default) {
                ("", Some(d)) => d.to_string(),
                (a, _) => a.to_string(),
            };
            match parse(&answer) {
                Ok(v) => return Ok(v),
                Err(e) => eprintln!("  {e}"),
            }
        }
    }

    fn finish(self) -> Result<(), String> {
        match self.flags.keys().next() {
            Some(k) => Err(format!("unknown flag --{k}")),
            None => Ok(()),
        }
    }
}

/// One `OracleFeedConfig`. `prefix` is `""` for `feed`, or `reflector-` /
/// `redstone-` for a `price-oracle` leg (whose provider is fixed).
fn feed_config(inputs: &mut Inputs, network: Network, provider: Provider, prefix: &str) -> Result<FeedConfig, String> {
    let label = match provider {
        Provider::Reflector => "Reflector",
        Provider::Redstone => "RedStone",
        Provider::Custom => "Oracle",
    };
    let known = provider.known_contract(network);
    let contract_key = format!("{prefix}contract");
    // A known deployment address applies without a prompt (it's a fixed,
    // verified fact); the staleness/tolerance defaults below are policy
    // choices, so outside a terminal those must always be explicit.
    let contract = match known {
        Some(addr) if !inputs.interactive && !inputs.flags.contains_key(&contract_key) => {
            eprintln!("note: --{contract_key} not given, using the known {label} {network:?} deployment {addr}");
            addr.to_string()
        }
        _ => inputs.get(
            &contract_key,
            &format!("{label} contract address"),
            known,
            contract_address,
        )?,
    };
    let asset = match provider.xlm_asset(network) {
        Some(asset) => asset,
        None => inputs.get("asset", "Asset (stellar:<C...> or other:<SYMBOL>)", None, Asset::parse)?,
    };
    let max_staleness_secs = inputs.get(
        &format!("{prefix}max-staleness-secs"),
        &format!("{label} max staleness (seconds; must be >= the oracle's resolution())"),
        Some("600"),
        max_staleness_secs,
    )?;
    let tolerance_bps = inputs.get(
        &format!("{prefix}tolerance-bps"),
        &format!("{label} tolerance vs Lazer (bps)"),
        Some("150"),
        tolerance_bps,
    )?;
    eprintln!("note: {label} {contract} with asset {asset}: initialize rejects max_staleness_secs {max_staleness_secs} if it is below that oracle's live resolution().");
    Ok(FeedConfig {
        contract,
        asset,
        max_staleness_secs,
        tolerance_bps,
    })
}

fn run(args: &[String]) -> Result<String, String> {
    let (command, rest) = args.split_first().ok_or("missing command (feed or price-oracle)")?;
    if command == "-h" || command == "--help" || command == "help" {
        return Ok(USAGE.to_string());
    }
    let mut inputs = Inputs::parse(rest)?;
    // Reject a typo'd flag before prompting for anything, not after.
    let allowed: &[&str] = match command.as_str() {
        "feed" => &[
            "network",
            "provider",
            "contract",
            "asset",
            "max-staleness-secs",
            "tolerance-bps",
        ],
        "price-oracle" => &[
            "network",
            "lazer-contract",
            "feed-id",
            "reflector-contract",
            "reflector-max-staleness-secs",
            "reflector-tolerance-bps",
            "redstone-contract",
            "redstone-max-staleness-secs",
            "redstone-tolerance-bps",
        ],
        other => return Err(format!("unknown command `{other}` (expected feed or price-oracle)")),
    };
    if let Some(k) = inputs.flags.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err(format!("unknown flag --{k} for `{command}`"));
    }
    let network = inputs.get("network", "Network (testnet or mainnet)", None, Network::parse)?;
    let json = match command.as_str() {
        "feed" => {
            let provider = inputs.get(
                "provider",
                "Provider (reflector, redstone or custom)",
                None,
                Provider::parse,
            )?;
            if provider != Provider::Custom && inputs.flags.contains_key("asset") {
                return Err(format!(
                    "--asset is only accepted with --provider custom (this provider always uses {})",
                    provider.xlm_asset(network).unwrap()
                ));
            }
            feed_config(&mut inputs, network, provider, "")?.to_json()
        }
        "price-oracle" => {
            let lazer_contract = inputs.get(
                "lazer-contract",
                "Pyth Lazer verifier contract address",
                None,
                contract_address,
            )?;
            let feed_id = inputs.get("feed-id", "Lazer feed id", None, feed_id)?;
            let reflector = feed_config(&mut inputs, network, Provider::Reflector, "reflector-")?;
            let redstone = feed_config(&mut inputs, network, Provider::Redstone, "redstone-")?;
            PriceOracleConfig {
                lazer_contract,
                feed_id,
                reflector,
                redstone,
            }
            .to_json()
        }
        _ => unreachable!("command checked above"),
    };
    inputs.finish()?;
    Ok(json.to_string())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(out) => {
            println!("{out}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}\n\nRun with --help for usage.");
            ExitCode::FAILURE
        }
    }
}
