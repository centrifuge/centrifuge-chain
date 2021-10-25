# Bridge Pallet

<!-- TOC -->

- [Bridge Pallet](#bridge-pallet)
    - [Overview](#overview)
    - [Pallet Usage](#pallet-usage)
        - [Add the Pallet to your Parachain Runtime](#add-the-pallet-to-your-parachain-runtime)
        - [Configure the Pallet](#configure-the-pallet)
    - [Pallet Documentation](#pallet-documentation)
    - [References](#references)
    - [License](#license)

<!-- /TOC -->

## Overview

This Substrate [FRAME v2](https://substrate.dev/docs/en/knowledgebase/runtime/frame) pallet 
provides functionalities for ...

## Pallet Usage

### Add the Pallet to your Parachain Runtime

In order to add this pallet to your runtime, you should add the following lines
to your parachain's main `Cargo.toml` file:

```toml
# -- snip --

[dependencies]

pallet-bridge = { branch = 'master', git = 'https://github.com/centrifuge-chain/pallet-bridge.git' }

# -- snip --

[features]
std = [
    # -- snip --
    'pallet-bridge/std',                # <-- Add this line
]
```

### Configure the Pallet

Now that the pallet is added to your runtime,  the latter must be configured
for your runtime (in `[runtime_path]/lib.rs` file):

```rust

node_primitives::Balance
use centrifuge_runtime::constants::currency;

// Parameterize bridge pallet
parameter_types! {
    pub const MinimalPayoutAmount: node_primitives::Balance = 5 * currency::RAD;
}

// Implement bridge pallet configuration trait for the mock runtime
impl pallet_rad_claims::Config for MyRuntime {
    type Event = ();
    type PalletId = RadClaimsPalletId;
    type Longevity = Longevity;
    type UnsignedPriority = UnsignedPriority;
    type AdminOrigin = EnsureSignedBy<One, u64>;
    type Currency = Balances;
}

construct_runtime! {
    …

    Bridge: pallet_bridge::{Pallet, Call, Config<T>, Storage, Event<T>},
}
```

## Pallet Documentation

You can see this pallet's reference documentation with the following command:

```sh
$ cargo doc --package pallet-bridge --open
```

The table of contents for this markdown file is automatically generated using the [`auto-markdown-toc`](https://marketplace.visualstudio.com/items?itemName=huntertran.auto-markdown-toc) extension for Visual StudioCode.

## References

## License
