# Upgrade Parachain
Simple Node JS script that upgrades the parachain against a local target

## Requirements
- yarn 1.22.x

## How to run
The script takes care of:

- Starting both relay-chain & parachain and waits until block production in both
- Stores authorizedUpgrade preimage from wasm provided (we could eventually read this from the release CI job step)
- Proposes council & democracy motions and votes on them
- Waits until referendum passes
- Enact Upgrade with wasm provided and waits until enacted by the relay chain
- Waits for 3 sessions (~3 mins) to ensure block production is unaltered

The script takes 3 arguments:
- The path of the WASM file to use for the upgrade
- The docker tag of the initial state of the centrifuge chain
- [Optional] The chain spec to use. Defaults to `centrifuge-local`

Steps:
1. Navigate to ./scripts/js/upgrade
2. Run `yarn` to install dependencies
3. Run `yarn execute $THE_TARGET_WASM $THE_GENESIS_DOCKER_TAG` to execute script

The process would take around 10 minutes to complete. 
Any timeout should be considered as error and should be looked further.