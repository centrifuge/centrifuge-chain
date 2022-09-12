<p align="center">
  <a href="https://github.com/centrifuge/centrifuge-chain">
    <img alt="Centrifuge" src="/docs/images/banner.png">
  </a>
  <h2 align="center">Centrifuge Chain</h2>

  <p align="center">
    The layer-1 blockchain for real-world assets, built on <a href="https://docs.substrate.io/" target="_blank">Substrate</a>.
    <br />
    <a href="https://docs.centrifuge.io/build/cent-chain/"><strong>Read the documentation »</strong></a>
  </p>
</p>

## About Centrifuge
Centrifuge Protocol is the Layer 1 (L1) protocol for bringing “real-world assets” (RWAs) on the blockchain creating the first global on-chain credit market. Centrifuge allows anyone to launch an on-chain credit fund creating collateral backed pools of assets. This on-chain securitization	 offers access to DeFi liquidity for any business and brings credit investment opportunities to DeFi protocols, institutional investors and retail investors alike.

Doing so will bring economic value from the real-world into DeFi, which in turn will help increase its mass adoption. Leveraging DeFi to finance these assets will be pivotal in building a more efficient, transparent, open, scalable financial system.

## Building blocks
The Substrate runtime makes use of various custom pallets that are found in the crates folder.

- [**pools**](https://github.com/centrifuge/centrifuge-chain/tree/parachain/pallets/pools) ([docs](https://reference.centrifuge.io/pallet_pools/index.html)): Preparing the chain for a new investment. It is bundling loans, slicing pools into tranches and managing investment epochs.

- [**loans**](https://github.com/centrifuge/centrifuge-chain/tree/parachain/pallets/loans) ([docs](https://reference.centrifuge.io/pallet_loans/index.html)): Locking a collateral NFT into a pool. The loans pallet is also used for bookkeeping its own value and outstanding debt.

- [**anchors**](https://github.com/centrifuge/centrifuge-chain/tree/parachain/pallets/anchors) ([docs](https://reference.centrifuge.io/pallet_anchors/index.html)): Storing hashes of documents on-chain. The documents are stored in the Private Off-chain Data (POD) node network.

- [**bridge**](https://github.com/centrifuge/centrifuge-chain/tree/parachain/pallets/bridge) ([docs](https://reference.centrifuge.io/pallet_bridge/index.html)): Connecting [ChainBridge](https://github.com/centrifuge/chainbridge-substrate) to transfer tranche tokens to and from Ethereum.

- [**bridge-mapping**](https://github.com/centrifuge/centrifuge-chain/tree/parachain/pallets/bridge-mapping) ([docs](https://reference.centrifuge.io/pallet_bridge_mapping/index.html)): Setting and tracking allowed paths for assets to be transferred across chains.

- [**claims**](https://github.com/centrifuge/centrifuge-chain/tree/parachain/pallets/claims) ([docs](https://reference.centrifuge.io/pallet_claims/index.html)): Processing claims of liquidity reward tokens acquired through Tinlake investments

- [**collator-allowlist**](https://github.com/centrifuge/centrifuge-chain/tree/parachain/pallets/collator-allowlist) ([docs](https://reference.centrifuge.io/pallet_collator_allowlist/index.html)): Tracking active collators, and allows the root account to manage this list

- [**crowdloan-claim**](https://github.com/centrifuge/centrifuge-chain/tree/parachain/pallets/crowdloan-claim) ([docs](https://reference.centrifuge.io/pallet_crowdloan_claim/index.html)): Claiming user rewards for their crowdloan funding support.

- [**crowdloan-rewards**](https://github.com/centrifuge/centrifuge-chain/tree/parachain/pallets/crowdloan-reward) ([docs](https://reference.centrifuge.io/pallet_crowdloan_reward/index.html)): Calculating the reward amounts for crowdloan contributors. This is used by the `crowdloan-claim` pallet which handles the actual claims.

- [**fees**](https://github.com/centrifuge/centrifuge-chain/tree/parachain/pallets/fees) ([docs](https://reference.centrifuge.io/pallet_fees/index.html)): Taking fees from interactions throughout the ecosystem.

- [**interest-accrual**](https://github.com/centrifuge/centrifuge-chain/tree/parachain/pallets/interest-accrual) ([docs](https://reference.centrifuge.io/pallet_interest_accrual/index.html)): Keeping account of the outstanding dept through interest accrual calcualations.

- [**keystore**](https://github.com/centrifuge/centrifuge-chain/tree/parachain/pallets/keystore) ([docs](https://reference.centrifuge.io/pallet_keystore/index.html)): Linking public keys to accounts.

- [**nft**](https://github.com/centrifuge/centrifuge-chain/tree/parachain/pallets/nft) ([docs](https://reference.centrifuge.io/pallet_nft/index.html)):
...

- [**nft-sales**](https://github.com/centrifuge/centrifuge-chain/tree/parachain/pallets/nft-sales) ([docs](https://reference.centrifuge.io/pallet_nft_sales/index.html)): Listing NFTs for sale and letting accounts buy NFTs.

- [**permissions**](https://github.com/centrifuge/centrifuge-chain/tree/parachain/pallets/permissions) ([docs](https://reference.centrifuge.io/pallet_permissions/index.html)): Linking roles to accounts. It is adding and removing relationships between roles and accounts on chain.

- [**restricted-tokens**](https://github.com/centrifuge/centrifuge-chain/tree/parachain/pallets/restricted-tokens) ([docs](https://reference.centrifuge.io/pallet_restricted_tokens/index.html)): Transferring tokens and setting balances. It is wrapping `orml-tokens` with the addition of checking for permissions.

## Contributions
Please follow the contributions guidelines as outlined in [`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md).

## License
This codebase is licensed under [GNU Lesser General Public License v3.0](https://github.com/centrifuge/centrifuge-chain/blob/parachain/LICENSE).
