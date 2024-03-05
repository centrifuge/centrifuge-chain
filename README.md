<div align="center">

![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/centrifuge/centrifuge-chain/build-wasm.yml?label=Actions&logo=github)
![GitHub Release](https://img.shields.io/github/v/release/centrifuge/centrifuge-chain)
[![Substrate version](https://img.shields.io/badge/Substrate-2.0.0-brightgreen?logo=Parity%20Substrate)](https://substrate.io/)
[![codecov](https://codecov.io/gh/centrifuge/centrifuge-chain/graph/badge.svg?token=RXlH3AJMRB)](https://codecov.io/gh/centrifuge/centrifuge-chain)
[![License](https://img.shields.io/github/license/centrifuge/centrifuge-chain?color=green)](https://github.com/centrifuge/centrifuge-chain/blob/main/LICENSE)
 <br />
[![Twitter URL](https://img.shields.io/twitter/url?style=social&url=https%3A%2F%2Ftwitter.com%2Fcentrifuge)](https://twitter.com/centrifuge/)
[![Telegram](https://img.shields.io/badge/Telegram-gray?logo=telegram)](https://t.me/centrifuge_chat)

</div>

<p align="center">
  <a href="https://centrifuge.io">
    <img alt="Centrifuge" src="/docs/images/banner.svg">
  </a>
  <h2 align="center">Centrifuge Chain</h2>

  <p align="center">
    The layer-1 blockchain for real-world assets, built on <a href="https://docs.substrate.io/">Substrate</a>.
    <br />
    <a href="https://docs.centrifuge.io/build/cent-chain/"><strong>Read the documentation »</strong></a>
  </p>

## About Centrifuge
Centrifuge is the infrastructure that facilitates the decentralized financing of real-world assets natively on-chain, creating a fully transparent market which allows borrowers and lenders to transact without unnecessary intermediaries. Asset pools are fully collateralized, liquidity providers have legal recourse, and the protocol is asset-class agnostic with pools for assets spanning mortgages, invoices, microlending and consumer finance. Ultimately, the protocol aims to lower the cost of borrowing for businesses around the world, while providing DeFi users with a stable source of collateralized yield that is uncorrelated to the volatile crypto markets. By bringing the entire structured credit market on-chain across securitization, tokenization, privacy, governance, and liquidity integrations, Centrifuge is building a more transparent, affordable, and limitless financial system.

## Building blocks
On top of the [Substrate FRAME](https://docs.substrate.io/reference/frame-pallets/) framework, Centrifuge Chain is composed of custom pallets which can be found inside the `pallets` folder. The following list gives a brief overview, and links to the corresponding documentation:

- [**pool-system**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/pool-system) ([docs](https://reference.centrifuge.io/pallet_pool_system/index.html)): Creating and managing investment pools. It is bundling loans, slicing pools into tranches, and controlling investment epochs.

- [**loans**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/loans) ([docs](https://reference.centrifuge.io/pallet_loans/index.html)): Locking a collateral NFT into a pool allowing to borrow from the pool. The loans pallet is also used for bookkeeping loan values and outstanding debt.

- [**anchors**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/anchors) ([docs](https://reference.centrifuge.io/pallet_anchors/index.html)): Storing hashes of documents on-chain. The documents are stored in the Private Off-chain Data (POD) node network.

- [**bridge**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/bridge) ([docs](https://reference.centrifuge.io/pallet_bridge/index.html)): Connecting [ChainBridge](https://github.com/centrifuge/chainbridge-substrate) to transfer tokens to and from Ethereum.

- [**collator-allowlist**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/collator-allowlist) ([docs](https://reference.centrifuge.io/pallet_collator_allowlist/index.html)): Tracking active collators, and allows the root account to manage this list.

- [**crowdloan-claim**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/crowdloan-claim) ([docs](https://reference.centrifuge.io/pallet_crowdloan_claim/index.html)): Claiming user rewards for their crowdloan funding support.

- [**crowdloan-rewards**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/crowdloan-reward) ([docs](https://reference.centrifuge.io/pallet_crowdloan_reward/index.html)): Calculating the reward amounts for crowdloan contributors. This is used by the `crowdloan-claim` pallet which handles the actual claims.

- [**fees**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/fees) ([docs](https://reference.centrifuge.io/pallet_fees/index.html)): Taking fees from accounts and sending this to the treasury, to the author, or burning them.

- [**interest-accrual**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/interest-accrual) ([docs](https://reference.centrifuge.io/pallet_interest_accrual/index.html)): Keeping account of the outstanding debt through interest accrual calculations.

- [**keystore**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/keystore) ([docs](https://reference.centrifuge.io/pallet_keystore/index.html)): Linking public keys to accounts. Supporting the operations of the offchain document consensus layer through the Centrifuge POD (Private Offchain Data) Node.

- [**nft**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/nft) ([docs](https://reference.centrifuge.io/pallet_nft/index.html)): Validating a mint request that needs to be transferred through the bridge layer to Ethereum.

- [**nft-sales**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/nft-sales) ([docs](https://reference.centrifuge.io/pallet_nft_sales/index.html)): Providing a place for digital art creators and owners to offer their NFTs for sale and for potential buyers to browse and buy them.

- [**permissions**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/permissions) ([docs](https://reference.centrifuge.io/pallet_permissions/index.html)): Linking roles to accounts. It is adding and removing relationships between roles and accounts on chain.

- [**restricted-tokens**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/restricted-tokens) ([docs](https://reference.centrifuge.io/pallet_restricted_tokens/index.html)): Transferring tokens and setting balances. It is wrapping `orml-tokens` with the addition of checking for permissions.

- [**rewards**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/rewards) ([docs](https://reference.centrifuge.io/pallet_rewards/index.html)): Implement a [scalable reward distribution](https://solmaz.io/2019/02/24/scalable-reward-changing/) mechanism that can be used for other pallets to create different rewards systems.

- [**liquidity-rewards**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/liquidity-rewards) ([docs](https://reference.centrifuge.io/pallet_liquidity_rewards/index.html)): Epoch based reward system.

## Developing
Instructions for building, testing, and developing Centrifuge Chain can be found in [`docs/DEVELOPING.md`](docs/DEVELOPING.md).

## License
This codebase is licensed under [GNU Lesser General Public License v3.0](https://github.com/centrifuge/centrifuge-chain/blob/main/LICENSE).
