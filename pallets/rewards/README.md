# Rewards Pallet

The Rewards pallet provides functionality for pull-based reward distributions,
implementing [these traits](https://reference.centrifuge.io/cfg_traits/rewards/index.html) as interface.

![image](https://user-images.githubusercontent.com/15687891/205727900-d578e336-5355-4b6a-8644-bbba004b2387.png)

The user can stake an amount to claim a proportional reward.
The staked amount is reserved/held from the user account for that currency when it's deposited
and unreserved/released when it's withdrawn.

The pallet stores three main entities:
- Groups, where the reward is distributed.
- Accounts, where the participants deposit and withdraw their stake in order to obtain the reward.
- Currencies, different stake types used in the same group.
These currencies can also be moved from one group to another,
in order to change the reward distribution of the associated accounts.

The pallet itself can be seen/understood as a wrapper for pull-based reward distributions.
The exact reward functionality of this pallet is configurable using a mechanism.
Mechanisms implement the reward methods.

**NOTE**: This pallet does not export any extrinsics, it's supposed to be used by other pallets directly or through the
[rewards traits](https://reference.centrifuge.io/cfg_traits/rewards/index.html) this pallet implements.

## Documentation

- [`pallet-rewards` API documentation](https://reference.centrifuge.io/pallet_rewards/)
- [Rewards traits API documentation](https://reference.centrifuge.io/cfg_traits/rewards/index.html)
- [The specifications](https://centrifuge.hackmd.io/@Luis/BJz0Ur2Mo) of the reward system.
- Mechanisms:
    - [base](https://solmaz.io/2019/02/24/scalable-reward-changing/) mechanism.
    - [deferred](https://centrifuge.hackmd.io/@Luis/SkB07jq8o) mechanism.
    - [gap](https://centrifuge.hackmd.io/@Luis/rkJXBz08s) mechanism

## Getting started

Add to your *Substrate* runtime or pallet `Cargo.toml`

```toml
[dependencies]
pallet-rewards = { git = "https://github.com/centrifuge/centrifuge-chain.git", branch = "release-vX.X.X", default-features = false }
```

Modify the `X.X.X` to a release that uses the same *Substrate* version as you use.

You probably will want to use this pallet as a [*loosely coupled pallet*](https://docs.substrate.io/build/pallet-coupling/).
For that, you need to add the interface traits as a dependency where you use them:

```toml
[dependencies]
cfg-traits = { git = "https://github.com/centrifuge/centrifuge-chain.git", branch = "release-vX.X.X", default-features = false }
```

*Take a look at the runtimes of this repository to see examples of how to configure it*
