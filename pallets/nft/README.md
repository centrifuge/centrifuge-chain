# NFT Pallet

<!-- TOC -->

- [NFT Pallet](#nft-pallet)
    - [Overview](#overview)
    - [Pallet Usage](#pallet-usage)
        - [Add the Pallet to your Parachain Runtime](#add-the-pallet-to-your-parachain-runtime)
        - [Configure the Pallet](#configure-the-pallet)
    - [Pallet Documentation](#pallet-documentation)
    - [References](#references)
    - [License](#license)

<!-- /TOC --

## Overview

This [FRAME](https://substrate.dev/docs/en/knowledgebase/runtime/frame) pallet 
provides functionalities for processing RAD token rewarding claims.

## Pallet Usage

### Add the Pallet to your Parachain Runtime

In order to add this pallet to your runtime, you should add the following lines
to your parachain's main `Cargo.toml` file:

```toml
# -- snip --

[dependencies]

pallet-nft = { branch = 'master', git = 'https://github.com/centrifuge/centrifuge-chain.git', default-features = false }

# -- snip --

[features]
std = [
    # -- snip --
    'pallet-nft/std',                # <-- Add this line
]
```

### Configure the Pallet

Now that the pallet is added to your runtime,  the latter must be configured
for your runtime (in `[runtime_path]/lib.rs` file):

```rust

// Parameterize Nft pallet
parameter_types! {
    pub const NftPalletId: PalletId = PalletId(*b"ccpa/nft");
}

// Implement bridge pallet configuration trait for the mock runtime
impl pallet_nft::Config for MyRuntime {
    type Event = ();
    type PalletId = NftPalletId;
}

construct_runtime! {
    …

    Nft: pallet_nft::{Pallet, Call, Config<T>, Storage, Event<T>},
}
```

## Pallet Documentation

You can see this pallet's reference documentation with the following command:

```sh
$ cargo doc --package pallet-nft --open
```

The table of contents for this markdown file is automatically generated using the [`auto-markdown-toc`](https://marketplace.visualstudio.com/items?itemName=huntertran.auto-markdown-toc) extension for Visual StudioCode.

## References

## License

GNU General Public License, Version 3, 29 June 2007 <https://www.gnu.org/licenses/gpl-3.0.html>
