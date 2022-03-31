# Claims Pallet

<!-- TOC -->

- [Claims Pallet](#claims-pallet)
    - [Overview](#overview)
    - [Pallet Usage](#pallet-usage)
        - [Add the Pallet to your Runtime](#add-the-pallet-to-your-runtime)
        - [Configure the Pallet](#configure-the-pallet)
    - [Pallet Documentation](#pallet-documentation)

<!-- /TOC -->

## Overview

This Centrifuge Chain pallet provides functionalities for processing claims of tokens acquired 
through [Tinlake](https://tinlake.centrifuge.io/) investments.

This pallet is built on Substrate [FRAME v2](https://substrate.dev/docs/en/knowledgebase/runtime/frame) 
library.

## Pallet Usage

### Add the Pallet to your Runtime

In order to add this pallet to your runtime, you should add the following lines
to your parachain's main `Cargo.toml` file:

```toml
# -- snip --

[dependencies]

pallet-claims = { branch = "master", git = "https://github.com/centrifuge/centrifuge-chain.git", default-features = false }

# -- snip --

[features]
std = [
    # -- snip --
    'pallet-claims/std',                # <-- Add this line
]
```

### Configure the Pallet

Now that the pallet is added to your runtime,  the latter must be configured
for your runtime (in `[runtime_path]/lib.rs` file):

```rust

node_primitives::Balance

// Centrifuge chain token definition
pub(crate) const MICRO_CFG: Balance = 1_000_000_000_000;    // 10−6 	0.000001
pub(crate) const MILLI_CFG: Balance = 1_000 * MICRO_CFG;    // 10−3 	0.001
pub(crate) const CENTI_CFG: Balance = 10 * MILLI_CFG;       // 10−2 	0.01
pub(crate) const CFG: Balance = 100 * CENTI_CFG;

// Parameterize claims pallet
parameter_types! {
    pub const ClaimsPalletId: PalletId = PalletId(*b"claims");
    pub const One: u64 = 1;
    pub const Longevity: u32 = 64;
    pub const MinimalPayoutAmount: node_primitives::Balance = 5 * CFG;
}

// Implement claims pallet configuration trait for the mock runtime
impl pallet_claims::Config for MyRuntime {
    type Event = ();
    type PalletId = ClaimsPalletId;
    type AdminOrigin = EnsureSignedBy<One, u64>;
    type Currency = Balances;
    type WeightInfo = ();
}

construct_runtime! {
    …

    Claims: pallet_claims::{Pallet, Call, Config<T>, Storage, Event<T>, ValidateUnsigned},
}
```

## Pallet Documentation

You can see this pallet's reference documentation with the following command:

```sh
$ cargo doc --package pallet-claims --open
```

The table of contents for this markdown file is automatically generated using the [`auto-markdown-toc`](https://marketplace.visualstudio.com/items?itemName=huntertran.auto-markdown-toc) extension for Visual StudioCode.
