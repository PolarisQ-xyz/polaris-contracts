# Security Policy

## Scope

This disclosure policy covers all Soroban smart contracts and companion Rust crates in `PolarisQ-xyz/polaris-contracts`:

- `contracts/market` (`polaris-market`)
- `contracts/perpetual` (`polaris-perpetual`)
- `contracts/vault` (`polaris-vault`)
- `contracts/smart-wallet` (`polaris-smart-wallet`) and `contracts/smart-wallet-factory` (`polaris-smart-wallet-factory`)
- `contracts/ctf-math` (`polaris-ctf-math`)
- `interface` (`polaris-interface`) and `tools/oracle-config` (`polaris-oracle-config`)

High-priority findings include any violation of the core collateral solvency invariant (`collateral.balance(contract) == total_supply`), unauthorized fund movement or admin bypass, oracle verification / corroboration bypass, WebAuthn origin or signature verification flaws, and permanent denial of service (traps or locked redemptions).

## Reporting a Vulnerability

**Do not open a public GitHub issue or pull request for suspected security vulnerabilities.**

Please report security findings privately through one of the following channels:

1. **GitHub Private Vulnerability Reporting (Preferred):** Use the [**Report a vulnerability**](https://github.com/PolarisQ-xyz/polaris-contracts/security/advisories/new) tab on this repository to open a private security advisory.
2. **Direct Maintainer Contact:** If GitHub Security Advisories are unavailable, contact the Polaris maintainers privately via the organization profile at [https://github.com/PolarisQ-xyz](https://github.com/PolarisQ-xyz).

When submitting a report, please include:

- The affected contract(s), entrypoint(s), and commit SHA.
- The exact invariant or authorization boundary that is violated.
- A minimal reproducible Soroban test case (`cargo test`) or step-by-step reproduction sequence demonstrating the impact.

## Response & Remediation Timeline

| Stage | Target SLA |
|---|---|
| Initial acknowledgment | Within **48 hours** of receipt |
| Triage & severity confirmation | Within **5 business days** |
| Fix / fresh-instance redeploy & coordinated disclosure | Within **14–30 days**, depending on active market expiry windows and user wallet migration requirements |

## Safe Harbor & Coordinated Disclosure

We ask that you give maintainers a reasonable window to remediate and (where required by Soroban's immutable-instance or factory-generation model) deploy replacement contract instances before making any details public. Good-faith security research conducted against local test environments or your own isolated testnet deployments will not be treated as hostile action.
