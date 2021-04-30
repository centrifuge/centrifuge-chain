# Crowdloan Reward Pallet

<!-- TOC -->

- [Crowdloan Reward Pallet](#crowdloan-reward-pallet)
  - [Overview](#overview)
  - [Pallet Usage](#pallet-usage)
    - [Add the Pallet to your Runtime](#add-the-pallet-to-your-runtime)
    - [Configure your Runtime](#configure-your-runtime)
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
parachain (i.e. on a parachain account). Otherwise, if the campaign miserably failed, the
locked DOTs are given back to the contributor.

This rewarding pallet aims at implementing the rewardind strategy, probably specific to
each parachain. The reward claim is delegated to the [`crowdloan_claim`] module.

## Pallet Usage

### Add the Pallet to your Runtime

In order to add this pallet to your runtime, you should add the following lines
to your parachain's main `Cargo.toml` file:

```toml
# -- snip --

[dependencies.pallet-crowdloan-claim]            # <-- Add the new dependency
default_features = false
git = 'https://github.com/centrifuge-chain/substrate-pallets-library.git'

# -- snip --

[features]
std = [
    # -- snip --
    'pallet-crowdloan-claim/std',                # <-- Add this line
]
```

### Configure your Runtime

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
