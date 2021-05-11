# Crowdloan Claim Pallet

<!-- TOC -->

- [Crowdloan Claim Pallet](#crowdloan-claim-pallet)
    - [Overview](#overview)
    - [Pallet Usage](#pallet-usage)
        - [Add the Pallet to your Parachain Runtime](#add-the-pallet-to-your-parachain-runtime)
        - [Configure the Pallet](#configure-the-pallet)
    - [Pallet Dependencies](#pallet-dependencies)
    - [Pallet Interface](#pallet-interface)
        - [Types Declaration](#types-declaration)
        - [Dispatchable Functions](#dispatchable-functions)
        - [Pallet Errors](#pallet-errors)
    - [Pallet Documentation](#pallet-documentation)
    - [References](#references)
    - [License](#license)

<!-- /TOC -->

## Overview

This pallet (or module) provides functionalities for rewarding contributors
who forgo staking on a Polkadot/Kusama relay chain so that to participate in
a commmunity-funding campaign, called [crowdloan](https://github.com/paritytech/polkadot/blob/master/runtime/common/src/crowdloan.rs),
for acquiring a parachain slot on the relay chain. For doing so, contributors 
lock DOTs on the Polkadot/Kusama relay chain, for the duration of the parachain 
slot auction compaign. If the campaign is successfully closed, for forgoing staking 
during the crowdloan campaign, contributors are rewarded in native tokens of the 
parachain (on a parachain account). Otherwise, if the campaign miserably fail, the
locked DOTs are given back to the contributor.

This rather "generic" Claim Module` acts as a proxy between the contributor
who claims a reward payout and the `Reward Pallet`, that concretely implement
the rewarding mechanism, using vesting or not, for instance. The two pallets
are loosely-coupled by means of the `RewardMechanism` trait (see Configure 
the Pallet below).

## Pallet Usage

### Add the Pallet to your Parachain Runtime

In order to add this pallet to your runtime, you should add the following lines
to your parachain's main `Cargo.toml` file:

```toml
# -- snip --

[dependencies.pallet-crowdloan-claim]            # <-- Add the new dependency
default_features = false
git = 'https://github.com/centrifuge-chain/pallet-crowdloan-claim.git'
branch = master

# -- snip --

[features]
std = [
    # -- snip --
    'pallet-crowdloan-claim/std',                # <-- Add this line
]
```

### Configure the Pallet

Now that the pallet is added to your runtime,  the latter must be configured
for your runtime (in `[runtime_path]/lib.rs` file):

```rust

construct_runtime! {
    …

    // Crowdloan campaign claim and reward payout processing pallets
    CrowdloanClaim: pallet_crowdloan_claim::{Module, Call, Config<T>, Storage, Event<T>, ValidateUnsigned},
    CrowdloanReward: pallet_crowdloan_reward::{Module, Call, Config, Storage, Event<T>},
}
```
## Pallet Dependencies

This pallet works hand in hand with the [`Reward Pallet`]().

## Pallet Interface

### Types Declaration

The following table describe associated types of this Pallet:

| Associated Type | Description |
| --------------- | ----------- |
| `WeightInfo` | Weight information for Pallet's dispatchable functions (or extrinsics) |

### Dispatchable Functions

This Pallet (or pallet)  provides the following dispatchable (or callable) functions:

| Function | Description | Error(s) |
| -------- | ---------- | ----------- | -------- |
| `initialize` | origin | … | … |
| `claim_reward` | origin | … | … |
| `claim_reward_with_identity_proof` | origin | … | … |

### Pallet Errors

This pallet exports the following errors:

| Error | Description |
| ----- | ----------- |
| `NotEnoughFunds` | … |

## Pallet Documentation

You can see this pallet's reference documentation with the following command:

```sh
$ cargo doc --package crowdloan-claim --open
```

The table of contents for this markdown file is automatically generated using the [`auto-markdown-toc`](https://marketplace.visualstudio.com/items?itemName=huntertran.auto-markdown-toc) extension for Visual StudioCode.

## References

## License
