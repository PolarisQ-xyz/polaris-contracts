# Build/test recipes for polaris-contracts.
#
# The build order below is not incidental: contracts/vault and
# contracts/smart-wallet-factory read sibling contracts' compiled wasm as
# *files* at macro-expansion time via `contractimport!`, a dependency Cargo's
# own build graph does not know about. polaris-market must therefore be built
# first, on its own, before anything else — including before `cargo test`.
# That rule lives here, once; CI and the docs call these recipes.

target := "wasm32v1-none"
market := "polaris-market"
rest := "-p polaris-mock-lazer -p polaris-mock-redstone -p polaris-smart-wallet -p polaris-smart-wallet-factory -p polaris-vault -p polaris-perpetual"

# Build every contract's release wasm, in the required order.
build-wasm:
    cargo build --release --target {{target}} -p {{market}}
    cargo build --release --target {{target}} {{rest}}

# Build the diagnostic (release-with-logs) wasm, same ordering rule.
build-wasm-logs:
    cargo build --profile release-with-logs --target {{target}} -p {{market}}
    cargo build --profile release-with-logs --target {{target}} {{rest}}

# Run the workspace test suite (builds the wasm it needs first).
test: build-wasm
    cargo test --workspace

# Print sha256 hashes of the release wasm artifacts.
hashes: build-wasm
    shasum -a 256 target/{{target}}/release/*.wasm

# Everything CI runs.
ci: test build-wasm-logs hashes
