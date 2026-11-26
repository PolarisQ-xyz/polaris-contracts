# Contributing

## Dependency version decisions

### `soroban-sdk` is pinned to `26.1.0`

The workspace `Cargo.toml` pins `soroban-sdk = "26.1.0"`. The latest published
release is `30.0.0-rc.1`, so we are several majors behind. This is tracked as a
deliberate decision (see issue #57), **not** neglect.

Unlike a typical crate, `soroban-sdk` major versions generally track specific
Stellar protocol versions. Jumping to a release candidate, or skipping several
majors, is not a simple bump and could easily be wrong to do reflexively.

**Do not bump `soroban-sdk` without first resolving the open questions below.**

#### Open questions

- Which Stellar protocol version does `26.1.0` correspond to?
- Which protocol versions do the later majors (`27.x`, `28.x`, `29.x`, `30.x`)
  map to?
- Has the testnet/mainnet network this project targets moved past the protocol
  version that `26.1.0` supports?

If staying on `26.1.0` matches the current target network's protocol, that is the
correct choice and should be recorded here. If the gap has grown unintentionally,
that should be captured as a follow-up before any bump.
