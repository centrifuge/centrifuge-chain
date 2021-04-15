# Claim Module for Crowdloan Campaign

<!-- TOC -->

- [Claim Module for Crowdloan Campaign](#claim-module-for-crowdloan-campaign)
  - [Overview](#overview)
  - [Module Usage](#module-usage)
    - [Add the Module to your Runtime](#add-the-module-to-your-runtime)
    - [Configure your Runtime](#configure-your-runtime)
  - [Module Dependencies](#module-dependencies)
  - [Module Interface](#module-interface)
    - [Types Declaration](#types-declaration)
    - [Dispatchable Functions](#dispatchable-functions)
    - [Module Errors](#module-errors)
  - [Module Documentation](#module-documentation)
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

This rather "generic" Claim Module` acts as a proxy between the contributor,
who claims a reward payout, and the `Reward Module`, that concretely implement
the rewarding strategy, using vesting or not, for instance.

## Module Usage

### Add the Module to your Runtime

In order to add this pallet to your runtime, you should add the following lines
to your parachain's main `Cargo.toml` file:

```toml
# -- snip --

[dependencies.crowdloan-claim]            # <-- Add the new dependency
default_features = false
git = 'https://github.com/centrifuge-chain/substrate-pallets-library.git'

# -- snip --

[features]
std = [
    # -- snip --
    'crowdloan-claim/std',                # <-- Add this line
]
```

### Configure your Runtime

## Module Dependencies

This pallet works hand in hand with the [`Reward Module`]().

## Module Interface

### Types Declaration

The following table describe associated types of this module:

| Associated Type | Description |
| --------------- | ----------- |
| `WeightInfo` | Weight information for module's dispatchable functions (or extrinsics) |

### Dispatchable Functions

This module (or pallet)  provides the following dispatchable (or callable) functions:

| Function | Description | Error(s) |
| -------- | ---------- | ----------- | -------- |
| `initialize` | origin | … | … |
| `claim_reward` | origin | … | … |
| `claim_reward_with_identity_proof` | origin | … | … |

### Module Errors

This module exports the following errors:

| Error | Description |
| ----- | ----------- |
| `NotEnoughFunds` | … |

## Module Documentation

You can see this module's reference documentation with the following command:

```sh
$ cargo doc --package crowdloan-claim --open
```

The table of contents for this markdown file is automatically generated using the [`auto-markdown-toc`](https://marketplace.visualstudio.com/items?itemName=huntertran.auto-markdown-toc) extension for Visual StudioCode.

## References

## License
