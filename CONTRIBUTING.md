# Contributing to polaris-contracts

This project follows an open contributor model: anyone is welcome to
contribute via peer review, testing, and patches. This document explains
the practical process, adapted from
[Bitcoin Core's CONTRIBUTING.md](https://github.com/bitcoin/bitcoin/blob/master/CONTRIBUTING.md)
— a small Rust/Soroban repo doesn't need everything a project the size of
Bitcoin Core does (no mailing list, no BIP process, no release branches to
backport into), but the underlying discipline — patches are small and
focused, reviewers actually test what they review, funds-critical code
gets a higher bar — applies here just as much as it does there.

There is no privileged "polaris-contracts developers" class. In practice
there's currently one maintainer reviewing and merging; that will change
as the project grows, and this document describes the process either way.

## Getting started

New contributors are welcome. **In-depth reviewing and testing is the
most effective way to start** — it teaches you the codebase faster than
writing a PR blind, and it's usually the bottleneck on a small project
like this one. See [Peer review](#peer-review) below.

Every open issue in this repo is written as a real problem found in the
shipped system — see the README's ["Cancellation
payout"](./README.md#cancellation-payout) and ["Factory wasm
pinning"](./README.md#factory-wasm-pinning--a-real-confirmed-exploitable-bug-that-shipped-fixed)
sections for what that looks like in practice, and each issue's own
"Context" section for precedent. Read the README section an issue
references before starting; it usually explains *why* the current
behavior is wrong, which matters more than the literal diff.

Before contributing, build the workspace and run the tests (see the
README's ["Building & testing"](./README.md#building--testing) section):

```sh
cargo test --workspace
cargo build --release --target wasm32v1-none -p polaris-market
```

## Communication

Discussion happens in GitHub issues and pull requests. There's no
separate chat/mailing list for a project this size — if you want early
feedback on an approach before writing code, open a draft PR or comment
on the issue.

## Contributor workflow

1. Fork the repository (first time only).
2. Create a topic branch.
3. Commit patches.
4. Push to your fork and open a pull request.

### Committing patches

Commits should be atomic and diffs easy to read — don't mix formatting
fixes or code moves with actual logic changes. Each commit should build
and pass `cargo test --workspace` on its own, not just at the tip of the
branch.

Commit messages should explain *why*, not just *what* — this codebase's
own commit history and README are full of "found live, fixed because X"
explanations; match that standard. Reference the issue a commit
addresses (`fixes #12`, `refs #12`).

### Creating the pull request

Prefix the PR title with the area it touches:

- `market` — `contracts/market`
- `perpetual` — `contracts/perpetual`
- `vault` — `contracts/vault`
- `wallet` — `contracts/smart-wallet` or `contracts/smart-wallet-factory`
- `oracle` — the Reflector/RedStone/Lazer integration code (`ctf-math`'s `sep40` module, the mocks)
- `docs` — README/comment-only changes
- `test` — test-only changes
- `ci`/`build` — workflow or Cargo config changes

Example: `vault: add fractional LP share accounting`

The PR description should explain what the patch does and, more
importantly, *why* — what problem it fixes, and how you tested it (which
new/existing tests cover it, or what you ran manually against testnet).
If there's reasonable doubt that you understand your own change or tested
it at a basic level, expect the PR to be closed rather than reviewed at
length — this isn't punitive, it's the same standard this repo holds its
own commits to (see the README's bug-hunting log for what "actually
verified" looks like here).

## Pull request philosophy

Keep patches focused: one PR fixes one bug, adds one feature, or does one
refactor — not a mixture. A refactor PR must not change behavior (bugs
get preserved as-is, fixed in a separate PR). Large, sprawling PRs are
harder to review and more likely to sit unreviewed.

**A higher bar applies to fund-safety-critical code** — anything in
`contracts/market`, `contracts/perpetual`, `contracts/vault`, or the
smart-wallet/factory pair that touches balances, share accounting, or
signature verification. This repo has a documented history of exactly
this class of bug (see the README's "Cancellation payout" and "Factory
wasm pinning" sections for two real, previously-shipped examples) — any
PR touching this surface should expect thorough review, not a quick
approve, and should preserve the solvency invariant
(`collateral_token.balance(contract) == contract.total_supply`) under
every ordering of calls.

## Error codes are append-only

Each contract's `Error` enum assigns a numeric value to every variant,
and those numbers are part of the contract's public ABI — integrators
(including `polaris-oracle` and `polaris-frontend`) map
`Error(Contract, #N)` to user-facing messages using them. The same number
means different things in different contracts, so mappings must be
per-contract.

**Never renumber an existing error variant.** Once a variant has shipped
with a numeric value, that value is permanent: do not change it, do not
remove the variant, and do not reuse its number. New variants are
appended with the next unused number. This keeps published mappings
stable and prevents a code from silently changing meaning.

When you add a variant, also add a row to the matching table in
[ERRORS.md](./ERRORS.md), which is the canonical per-contract reference
(code, variant, meaning, returning entrypoints, suggested user message).

## Changing storage

Storage keys, storage classes (instance vs persistent), and TTL bump
policies are documented per contract in [docs/storage.md](./docs/storage.md).
Before adding or changing any storage key or stored struct:

- [ ] Read [docs/storage.md](./docs/storage.md) and confirm you are not
  changing the shape of an already-deployed struct (add a new key instead).
- [ ] Add a row for any new key to the matching table in
  [docs/storage.md](./docs/storage.md), including its storage class, value
  type, and TTL bump policy.
- [ ] If the key is persistent, note it so archival/restore planning picks
  it up.

## Peer review

Anyone may review a pull request via comments. A review typically covers
whether the change is a good idea at all (concept), whether the approach
is right, and whether the code itself is correct.

- **`Concept (N)ACK`** — "I do (not) agree with the goal of this PR."
- **`Approach (N)ACK`** — "I agree with the goal, but (not) with how this
  achieves it."
- **`ACK <commit>`** — code review, plus a note on how you reviewed it:
  "I tested this against testnet by X" or "I read it and it looks
  correct, didn't run it."

A `NACK` needs a reason — an unexplained NACK can be disregarded.
"Nit" means a trivial, non-blocking issue (a typo, a naming preference) —
don't block a merge over one.

If you say you tested something, say how — "ran `cargo test --workspace`"
is different from "deployed to testnet and confirmed X on-chain," and
readers of the PR (including future contributors trying to understand why
a change was trusted) benefit from knowing which one happened.

## Decision making

Whether a PR merges is the maintainer's call, informed by peer review.
In general, a PR should:

- Fix a real, demonstrated problem or serve a clear purpose (not a
  cosmetic preference with no functional benefit).
- Include tests that would fail without the fix and pass with it — the
  same bar this repo's own bug-hunting log holds itself to.
- Not break `cargo test --workspace`.
- Update the README/doc comments if it changes documented behavior.

## Copyright

By contributing, you agree to license your work under the [MIT
license](./LICENSE), the same license this repository is distributed
under.
