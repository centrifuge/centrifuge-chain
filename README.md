<div align="center">

[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/centrifuge/centrifuge-chain/build-wasm.yml?label=Actions&logo=github)](https://github.com/centrifuge/centrifuge-chain/actions)
[![codecov](https://codecov.io/gh/centrifuge/centrifuge-chain/graph/badge.svg?token=RXlH3AJMRB)](https://codecov.io/gh/centrifuge/centrifuge-chain)
[![GitHub Release](https://img.shields.io/github/v/release/centrifuge/centrifuge-chain)](https://github.com/centrifuge/centrifuge-chain/releases)
[![Substrate version](https://img.shields.io/badge/Substrate-2.0.0-brightgreen?logo=Parity%20Substrate)](https://substrate.io/)
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

- [**anchors**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/anchors) ([docs](https://reference.centrifuge.io/pallet_anchors/index.html)): Storing hashes of documents on-chain. The documents are stored in the Private Off-chain Data (POD) node network.
- 
- [**anchors-v2**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/anchors-v2) ([docs](https://reference.centrifuge.io/pallet_anchors_v2/index.html)): Second version of the pallet used to store document hashes on-chain.

- [**block-rewards**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/block-rewards) ([docs](https://reference.centrifuge.io/pallet_block_rewards/index.html)): Provides means of configuring and distributing block rewards to collators as well as the annual treasury inflation.

- [**bridge**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/bridge) ([docs](https://reference.centrifuge.io/pallet_bridge/index.html)): Connecting [ChainBridge](https://github.com/centrifuge/chainbridge-substrate) to transfer tokens to and from Ethereum.

- [**collator-allowlist**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/collator-allowlist) ([docs](https://reference.centrifuge.io/pallet_collator_allowlist/index.html)): Tracking active collators, and allows the root account to manage this list.

- [**ethereum-transaction**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/ethereum-transaction) ([docs](https://reference.centrifuge.io/pallet_ethereum_transaction/index.html)): Wrapper around the Ethereum pallet which allows other pallets to execute EVM calls.

- [**fees**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/fees) ([docs](https://reference.centrifuge.io/pallet_fees/index.html)): Taking fees from accounts and sending this to the treasury, to the author, or burning them.

- [**foreign-investments**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/foreign-investments) ([docs](https://reference.centrifuge.io/pallet_foreign_investments/index.html)): Enables investing, redeeming and collecting in foreign and non-foreign currencies. Can be regarded as an extension of `pallet-investments` which provides the same toolset for pool (non-foreign) currencies.

- [**interest-accrual**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/interest-accrual) ([docs](https://reference.centrifuge.io/pallet_interest_accrual/index.html)): Keeping account of the outstanding debt through interest accrual calculations.

- [**investments**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/investments) ([docs](https://reference.centrifuge.io/pallet_investments/index.html)): Provides orders for assets and allows user to collect these orders.

- [**keystore**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/keystore) ([docs](https://reference.centrifuge.io/pallet_keystore/index.html)): Linking public keys to accounts. Supporting the operations of the offchain document consensus layer through the Centrifuge POD (Private Offchain Data) Node.

- [**liquidity-pools**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/liquidity-pools) ([docs](https://reference.centrifuge.io/pallet_liquidity_pools/index.html)): Provides the toolset to enable foreign investments on foreign domains.

- [**liquidity-pools-gateway**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/liquidity-pools-gateway) ([docs](https://reference.centrifuge.io/pallet_liquidity_pools_gateway/index.html)): The main handler of incoming and outgoing Liquidity Pools messages.

- [**liquidity-pools-gateway-routers**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/liquidity-pools-gateway/routers) ([docs](https://reference.centrifuge.io/liquidity_pools_gateway_routers/index.html)): This crate contains the `DomainRouters` used by the Liquidity Pools Gateway pallet.

- [**axelar-gateway-precompile**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/liquidity-pools-gateway/axelar-gateway-precompile) ([docs](https://reference.centrifuge.io/axelar_gateway_precompile/index.html)): Pallet that serves as an EVM precompile for incoming Liquidity Pools messages from the Axelar network.

- [**liquidity-rewards**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/liquidity-rewards) ([docs](https://reference.centrifuge.io/pallet_liquidity_rewards/index.html)): Epoch based reward system.

- [**loans**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/loans) ([docs](https://reference.centrifuge.io/pallet_loans/index.html)): Locking a collateral NFT into a pool allowing to borrow from the pool. The loans pallet is also used for bookkeeping loan values and outstanding debt.

- [**oracle-collection**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/oracle-collection) ([docs](https://reference.centrifuge.io/pallet_oracle_collection/index.html)): Pallet used to collect and aggregate oracle values.

- [**oracle-feed**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/oracle-feed) ([docs](https://reference.centrifuge.io/pallet_oracle_feed/index.html)): Pallet used to feed oracle values.

- [**order-book**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/order-book) ([docs](https://reference.centrifuge.io/pallet_order_book/index.html)): Allows orders for currency swaps to be placed and fulfilled.

- [**permissions**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/permissions) ([docs](https://reference.centrifuge.io/pallet_permissions/index.html)): Linking roles to accounts. It is adding and removing relationships between roles and accounts on chain.

- [**pool-fees**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/pool-fees) ([docs](https://reference.centrifuge.io/pallet_pool_fees/index.html)): Stores all the fees related to a pool and allows for these fees to be charged.

- [**pool-registry**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/pool-registry) ([docs](https://reference.centrifuge.io/pallet_pool_registry/index.html)): Used for creating, updating, and setting the metadata of pools.

- [**pool-system**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/pool-system) ([docs](https://reference.centrifuge.io/pallet_pool_system/index.html)): Creating and managing investment pools. It is bundling loans, slicing pools into tranches, and controlling investment epochs.

- [**restricted-tokens**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/restricted-tokens) ([docs](https://reference.centrifuge.io/pallet_restricted_tokens/index.html)): Transferring tokens and setting balances. It is wrapping `orml-tokens` with the addition of checking for permissions.

- [**restricted-xtokens**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/restricted-xtokens) ([docs](https://reference.centrifuge.io/pallet_restricted_xtokens/index.html)): Wrapper pallet over `orml-xtokens` which allows the runtime to create arbitrary filters for transfers of x-chain-transfers.

- [**rewards**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/rewards) ([docs](https://reference.centrifuge.io/pallet_rewards/index.html)): Implement a [scalable reward distribution](https://solmaz.io/2019/02/24/scalable-reward-changing/) mechanism that can be used for other pallets to create different rewards systems.

- [**swaps**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/swaps) ([docs](https://reference.centrifuge.io/pallet_swaps/index.html)): Enables applying swaps independently of previous swaps in the same or opposite directions.

- [**token-mux**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/token-mux) ([docs](https://reference.centrifuge.io/pallet_token_mux/index.html)): Enables proxying variants of the same foreign assets to a local asset representation.

- [**transfer-allowlist**](https://github.com/centrifuge/centrifuge-chain/tree/main/pallets/transfer-allowlist) ([docs](https://reference.centrifuge.io/pallet_transfer_allowlist/index.html)): This pallet checks whether an account should be allowed to make a transfer to a receiving location with a specific currency.

## Developing
Instructions for building, testing, and developing Centrifuge Chain can be found in [`docs/DEVELOPING.md`](docs/DEVELOPING.md).

## License
This codebase is licensed under [GNU Lesser General Public License v3.0](https://github.com/centrifuge/centrifuge-chain/blob/main/LICENSE).
