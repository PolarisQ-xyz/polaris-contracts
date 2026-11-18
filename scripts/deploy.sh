#!/usr/bin/env bash
# Deploy and initialize a Polaris contract on a given network.
#
# Usage: scripts/deploy.sh <network> <contract>
#
#   <network>   testnet | mainnet | futurenet | local
#   <contract>  market | vault | smart-wallet-factory | mock-redstone | mock-lazer
#
# Built on the stellar CLI. For the factory this performs the full
# upload -> capture wasm hash -> initialize(admin, wasm_hash, origin)
# sequence. Results are written to deployments/<network>.json.
#
# Mock contracts (mock-redstone, mock-lazer) are refused on any network
# other than testnet.
set -euo pipefail

NETWORK="${1:-}"
CONTRACT="${2:-}"

if [[ -z "$NETWORK" || -z "$CONTRACT" ]]; then
  echo "usage: $0 <network> <contract>" >&2
  echo "  networks:  testnet | mainnet | futurenet | local" >&2
  echo "  contracts: market | vault | smart-wallet-factory | mock-redstone | mock-lazer" >&2
  exit 1
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEPLOYMENTS_DIR="$REPO_ROOT/deployments"
DEPLOYMENTS_FILE="$DEPLOYMENTS_DIR/$NETWORK.json"

# Map network name to the stellar CLI network flag.
case "$NETWORK" in
  testnet)   CLI_NETWORK="testnet" ;;
  mainnet)   CLI_NETWORK="mainnet" ;;
  futurenet) CLI_NETWORK="futurenet" ;;
  local)     CLI_NETWORK="local" ;;
  *)
    echo "error: unknown network '$NETWORK'" >&2
    exit 1
    ;;
esac

# Refuse to deploy mock contracts anywhere but testnet.
case "$CONTRACT" in
  mock-redstone|mock-lazer)
    if [[ "$NETWORK" != "testnet" ]]; then
      echo "error: refusing to deploy mock contract '$CONTRACT' on '$NETWORK' (testnet only)" >&2
      exit 1
    fi
    ;;
esac

# Resolve the source identity (stellar CLI uses the configured identity).
SOURCE="${STELLAR_SOURCE:-default}"

# Map contract name to its wasm artifact path.
case "$CONTRACT" in
  market)              WASM="$REPO_ROOT/target/wasm32-unknown-unknown/release/polaris_market.wasm" ;;
  vault)               WASM="$REPO_ROOT/target/wasm32-unknown-unknown/release/polaris_vault.wasm" ;;
  smart-wallet-factory) WASM="$REPO_ROOT/target/wasm32-unknown-unknown/release/polaris_smart_wallet_factory.wasm" ;;
  mock-redstone)       WASM="$REPO_ROOT/target/wasm32-unknown-unknown/release/polaris_mock_redstone.wasm" ;;
  mock-lazer)          WASM="$REPO_ROOT/target/wasm32-unknown-unknown/release/polaris_mock_lazer.wasm" ;;
  *)
    echo "error: unknown contract '$CONTRACT'" >&2
    exit 1
    ;;
esac

if [[ ! -f "$WASM" ]]; then
  echo "error: wasm not found at $WASM (build it first)" >&2
  exit 1
fi

mkdir -p "$DEPLOYMENTS_DIR"

# Upload the wasm and capture its hash.
echo "uploading $CONTRACT wasm..."
WASM_HASH="$(stellar contract upload \
  --network "$CLI_NETWORK" \
  --source "$SOURCE" \
  --wasm "$WASM")"
echo "wasm hash: $WASM_HASH"

# Deploy the contract instance.
echo "deploying $CONTRACT..."
CONTRACT_ID="$(stellar contract deploy \
  --network "$CLI_NETWORK" \
  --source "$SOURCE" \
  --wasm-hash "$WASM_HASH")"
echo "contract id: $CONTRACT_ID"

# Initialize where required.
case "$CONTRACT" in
  smart-wallet-factory)
    ADMIN="${FACTORY_ADMIN:-$SOURCE}"
    ORIGIN="${FACTORY_ORIGIN:-}"
    if [[ -z "$ORIGIN" ]]; then
      echo "error: FACTORY_ORIGIN must be set to initialize the factory" >&2
      exit 1
    fi
    echo "initializing factory (admin=$ADMIN, origin=$ORIGIN)..."
    stellar contract invoke \
      --network "$CLI_NETWORK" \
      --source "$SOURCE" \
      --id "$CONTRACT_ID" \
      -- initialize \
      --admin "$ADMIN" \
      --wasm_hash "$WASM_HASH" \
      --origin "$ORIGIN"
    ;;
  market|vault)
    echo "note: $CONTRACT deployed; run its initialize via polaris-oracle or manually"
    ;;
  mock-redstone|mock-lazer)
    echo "note: $CONTRACT deployed (mock, testnet only)"
    ;;
esac

# Record the result in deployments/<network>.json.
if [[ -f "$DEPLOYMENTS_FILE" ]]; then
  # Merge the new entry into the existing JSON object.
  TMP="$(mktemp)"
  jq --arg name "$CONTRACT" --arg id "$CONTRACT_ID" --arg hash "$WASM_HASH" \
    '.[$name] = {contract_id: $id, wasm_hash: $hash}' \
    "$DEPLOYMENTS_FILE" > "$TMP"
  mv "$TMP" "$DEPLOYMENTS_FILE"
else
  jq -n --arg name "$CONTRACT" --arg id "$CONTRACT_ID" --arg hash "$WASM_HASH" \
    '{($name): {contract_id: $id, wasm_hash: $hash}}' \
    > "$DEPLOYMENTS_FILE"
fi

echo "wrote $DEPLOYMENTS_FILE"
