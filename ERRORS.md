# Error-code reference

Every Polaris contract returns errors as `Error(Contract, #N)`, where `N` is
the numeric value of the contract's `Error` enum variant. **The same numeric
code means different things in different contracts** — always resolve a code
against the table for the contract that emitted it.

## Stability policy

Error codes are **append-only**. Existing numeric values are never renumbered
or reused, even if a variant is renamed or deprecated. New variants are added
with the next unused number. See `CONTRIBUTING.md`.

## `polaris-market`

| Code | Variant | Meaning | Returned by | Suggested user message |
| ---: | --- | --- | --- | --- |
| 1 | `NotInitialized` | Contract has not been initialized | any entrypoint before `initialize` | "Market is not set up yet." |
| 2 | `AlreadyInitialized` | `initialize` called twice | `initialize` | "Market is already set up." |
| 3 | `InvalidStrikePrice` | Strike price out of range | `initialize` | "Invalid strike price." |
| 4 | `InvalidExpiry` | Expiry not in the future | `initialize` | "Invalid expiry." |
| 5 | `InvalidGracePeriod` | Grace period out of range | `initialize` | "Invalid grace period." |
| 6 | `InvalidFeedId` | Pyth Lazer feed id invalid | `initialize` | "Invalid price feed." |
| 7 | `InvalidFeeBps` | Fee above `MAX_FEE_BPS` | `initialize` | "Fee is too high." |
| 8 | `InvalidLiquidity` | Initial liquidity invalid | `initialize` | "Invalid initial liquidity." |
| 9 | `MarketNotOpen` | Market is not in `Open` status | `split`, `merge`, `buy`, `sell`, `transfer` | "This market is no longer open." |
| 10 | `TradingClosed` | Trading window has closed | `buy`, `sell` | "Trading has closed for this market." |
| 11 | `InvalidAmount` | Amount is zero or otherwise invalid | `split`, `merge`, `buy`, `sell`, `transfer`, `redeem` | "Enter a valid amount." |
| 12 | `InsufficientBalance` | Caller lacks the shares/collateral | `merge`, `sell`, `transfer`, `redeem` | "Insufficient balance." |
| 13 | `SlippageExceeded` | Trade output below `min_out` | `buy`, `sell` | "Price moved — slippage limit exceeded." |
| 14 | `ExpiryNotReached` | `settle` called before expiry | `settle` | "Market has not expired yet." |
| 15 | `InvalidPayload` | Malformed oracle update payload | `settle` | "Invalid price update." |
| 16 | `FeedNotFound` | Feed id not present in update | `settle` | "Price feed not found in update." |
| 17 | `StalePrice` | Price older than freshness window | `settle` | "Price update is stale." |
| 18 | `FuturePrice` | Price timestamped in the future | `settle` | "Price update timestamp is invalid." |
| 19 | `GracePeriodNotElapsed` | `cancel` before `expiry + grace` | `cancel` | "Grace period has not elapsed." |
| 20 | `AlreadyFinalized` | Market already resolved/cancelled | `settle`, `cancel` | "Market is already finalized." |
| 21 | `NotFinalized` | Market not yet resolved/cancelled | `redeem` | "Market is not finalized yet." |
| 22 | `NothingToRedeem` | Caller holds no winning shares | `redeem` | "Nothing to redeem." |
| 23 | `PoolDepthExceeded` | Trade would drain an AMM reserve | `buy`, `sell` | "Trade is too large for the current pool." |
| 24 | `ReflectorPriceUnavailable` | Reflector returned no price | `settle` | "Secondary oracle price unavailable." |
| 25 | `ReflectorPriceStale` | Reflector price too old | `settle` | "Secondary oracle price is stale." |
| 26 | `ReflectorDecimalsInvalid` | Reflector decimals out of range | `settle` | "Secondary oracle misconfigured." |
| 27 | `OracleDivergence` | Lazer and secondary oracle disagree | `settle` | "Oracles disagree — settlement rejected." |
| 28 | `InvalidReflectorConfig` | Reflector config invalid at init | `initialize` | "Invalid secondary oracle config." |
| 29 | `ReflectorDecimalsChanged` | Reflector decimals changed since init | `settle` | "Secondary oracle changed — settlement rejected." |
| 30 | `RedstonePriceUnavailable` | RedStone returned no price | `settle` | "Third oracle price unavailable." |
| 31 | `RedstonePriceStale` | RedStone price too old | `settle` | "Third oracle price is stale." |
| 32 | `RedstoneDecimalsInvalid` | RedStone decimals out of range | `settle` | "Third oracle misconfigured." |
| 33 | `RedstoneDecimalsChanged` | RedStone decimals changed since init | `settle` | "Third oracle changed — settlement rejected." |
| 34 | `InvalidRedstoneConfig` | RedStone config invalid at init | `initialize` | "Invalid third oracle config." |
| 35 | `Paused` | Admin paused trading | `buy`, `sell` | "Trading is paused." |

## `polaris-perpetual`

| Code | Variant | Meaning | Returned by | Suggested user message |
| ---: | --- | --- | --- | --- |
| 1 | `NotInitialized` | Contract not initialized | any entrypoint before `initialize` | "Market is not set up yet." |
| 2 | `AlreadyInitialized` | `initialize` called twice | `initialize` | "Market is already set up." |
| 3 | `InvalidStrikePrice` | Strike price out of range | `initialize` | "Invalid strike price." |
| 4 | `InvalidExpiry` | Expiry not in the future | `initialize` | "Invalid expiry." |
| 5 | `InvalidGracePeriod` | Grace period out of range | `initialize` | "Invalid grace period." |
| 6 | `MarketNotOpen` | Market is not in `Open` status | `open_position`, `close_position`, `deposit`, `withdraw` | "This market is no longer open." |
| 7 | `InvalidAmount` | Amount is zero or otherwise invalid | `open_position`, `close_position`, `deposit`, `withdraw`, `liquidate` | "Enter a valid amount." |
| 8 | `InsufficientBalance` | Caller lacks margin/collateral | `close_position`, `withdraw`, `liquidate` | "Insufficient balance." |
| 9 | `SlippageExceeded` | Execution price outside limit | `open_position`, `close_position` | "Price moved — slippage limit exceeded." |
| 10 | `ExpiryNotReached` | `settle` before expiry | `settle` | "Market has not expired yet." |
| 11 | `InvalidPayload` | Malformed oracle update payload | `settle` | "Invalid price update." |
| 12 | `FeedNotFound` | Feed id not present in update | `settle` | "Price feed not found in update." |
| 13 | `StalePrice` | Price older than freshness window | `settle` | "Price update is stale." |
| 14 | `FuturePrice` | Price timestamped in the future | `settle` | "Price update timestamp is invalid." |
| 15 | `GracePeriodNotElapsed` | `cancel` before `expiry + grace` | `cancel` | "Grace period has not elapsed." |
| 16 | `AlreadyFinalized` | Market already resolved/cancelled | `settle`, `cancel` | "Market is already finalized." |
| 17 | `NotFinalized` | Market not yet resolved/cancelled | `withdraw` | "Market is not finalized yet." |
| 18 | `PositionNotFound` | No open position for caller | `close_position`, `liquidate` | "No open position." |
| 19 | `NotLiquidatable` | Position is above maintenance margin | `liquidate` | "Position is not liquidatable." |
| 20 | `OracleDivergence` | Lazer and secondary oracle disagree | `settle` | "Oracles disagree — settlement rejected." |
| 21 | `ReflectorPriceUnavailable` | Reflector returned no price | `settle` | "Secondary oracle price unavailable." |
| 22 | `ReflectorPriceStale` | Reflector price too old | `settle` | "Secondary oracle price is stale." |
| 23 | `ReflectorDecimalsInvalid` | Reflector decimals out of range | `settle` | "Secondary oracle misconfigured." |
| 24 | `ReflectorDecimalsChanged` | Reflector decimals changed since init | `settle` | "Secondary oracle changed — settlement rejected." |
| 25 | `InvalidReflectorConfig` | Reflector config invalid at init | `initialize` | "Invalid secondary oracle config." |
| 26 | `InvalidFeeBps` | Fee above `MAX_FEE_BPS` | `initialize` | "Fee is too high." |
| 27 | `InvalidLiquidity` | Initial liquidity invalid | `initialize` | "Invalid initial liquidity." |
| 28 | `InvalidFeedId` | Pyth Lazer feed id invalid | `initialize` | "Invalid price feed." |
| 29 | `PoolDepthExceeded` | Trade would drain the AMM reserve | `open_position`, `close_position` | "Trade is too large for the current pool." |
| 30 | `InvalidLeverage` | Leverage outside allowed range | `open_position` | "Invalid leverage." |
| 31 | `Paused` | Admin paused trading | `open_position`, `close_position` | "Trading is paused." |

## `polaris-vault`

| Code | Variant | Meaning | Returned by | Suggested user message |
| ---: | --- | --- | --- | --- |
| 1 | `NotInitialized` | Vault not initialized | any entrypoint before `initialize` | "Vault is not set up yet." |
| 2 | `AlreadyInitialized` | `initialize` called twice | `initialize` | "Vault is already set up." |
| 3 | `InvalidAmount` | Deposit/withdraw amount invalid | `deposit`, `withdraw` | "Enter a valid amount." |
| 4 | `InsufficientShares` | Caller lacks vault shares | `withdraw` | "Insufficient shares." |
| 5 | `InsufficientBalance` | Vault lacks underlying balance | `withdraw` | "Insufficient balance." |
| 6 | `Unauthorized` | Caller is not the admin | admin-only entrypoints | "Not authorized." |
| 7 | `Paused` | Admin paused deposits/withdrawals | `deposit`, `withdraw` | "Vault is paused." |

## `polaris-smart-wallet`

| Code | Variant | Meaning | Returned by | Suggested user message |
| ---: | --- | --- | --- | --- |
| 1 | `NotInitialized` | Wallet not initialized | any entrypoint before `initialize` | "Wallet is not set up yet." |
| 2 | `AlreadyInitialized` | `initialize` called twice | `initialize` | "Wallet is already set up." |
| 3 | `Unauthorized` | Signer not authorized | `execute`, `add_signer`, `remove_signer` | "Not authorized." |
| 4 | `InvalidSignature` | Signature verification failed | `execute` | "Invalid signature." |
| 5 | `SignerNotFound` | Signer does not exist | `remove_signer` | "Signer not found." |
| 6 | `TooManySigners` | Signer set is full | `add_signer` | "Too many signers." |
| 7 | `InvalidThreshold` | Threshold out of range | `initialize`, `set_threshold` | "Invalid signature threshold." |
| 8 | `ReplayDetected` | Nonce already used | `execute` | "Transaction already submitted." |

## `polaris-smart-wallet-factory`

| Code | Variant | Meaning | Returned by | Suggested user message |
| ---: | --- | --- | --- | --- |
| 1 | `NotInitialized` | Factory not initialized | any entrypoint before `initialize` | "Factory is not set up yet." |
| 2 | `AlreadyInitialized` | `initialize` called twice | `initialize` | "Factory is already set up." |
| 3 | `Unauthorized` | Caller is not the admin | admin-only entrypoints | "Not authorized." |
| 4 | `InvalidSalt` | Deployment salt already used | `deploy` | "Wallet already exists for this salt." |
| 5 | `DeployFailed` | Underlying deploy call failed | `deploy` | "Wallet deployment failed." |
| 6 | `WalletNotFound` | No wallet registered for address | `lookup` | "Wallet not found." |
