# XCM Research

This directory contains the artefacts necessary for the research and development
of XCM features in the Centrifuge chain.


## Local environment

The local environment we setup is composed of:

`Relay-Chain`
	`|——>` `Statemine-local Parachain`
	`|——>` `Cfg Development Parachain`

### Start 

To start the local environment, follow these steps.

0. Copy Cumulus' `polkadot-collator` to `xcm/bin`

    You will need to clone and build cumulus and copy the `polkadot-collator`
    executable.

    NOTE: we use cumulus' revision `c02514d8` for now.

   ``` bash
   git clone https://github.com/paritytech/cumulus
   cd cumulus
   git checkout c02514d8
   cargo build --release

   cp ./target/release/polkadot-collator ../xcm/bim
   ```

1. Start the relay chain
   `./scripts/init.sh start-relay-chain`

2. Start the cfg parachain
    `./scripts/init.sh start-parachain purge`

3. Start the `statemine-local` parachain

    Copy the content of the `./xcm/bootnodes` file and replace the `<bootnodes>` bit below with said contents.

    ``` bash
    ./xcm/bin/polkadot-collator \
    --collator --alice --force-authoring --tmp \
    --chain statemine-local --parachain-id 42 \
    --port 40335 --ws-port 9947 \
    -- \
    --execution wasm \
    --chain ./res/rococo-local.json \
    --port 30335 \
    <bootnodes>
    ```

4. Onboard the cfg parachain

5. Onboard the `statemine-local` parachain

   5.1 Export the genesis state 
   `./bin/polkadot-collator export-genesis-state --chain statemine-local --parachain-id 42 > statemine-local-genesis-state`

   5.2 Export the genesis wasm
   `./bin/target/release/polkadot-collator export-genesis-wasm --chain statemine-local  > statemine-local-genesis-wasm`
