# Testing a runtime upgrade in local
This must be done before each new release we do in any of out runtimes.
Supposing we are testing centrifuge runtime in local:

1. Run the relay chain:
    ```sh
    ./scripts/init.sh start-relay-chain
    ```
    Open **Rococo Local Testnet** client [here](https://polkadot.js.org/apps/?rpc=ws%3A%2F%2F127.0.0.1%3A9944#/explorer)

1. Run the parachain using the last available release:
    ```sh
    PARA_CHAIN_SPEC=centrifuge-local CC_DOCKER_TAG=<DOCKER_TAG> ./scripts/init.sh start-parachain-docker
    ```
    The `DOCKER_TAG` has the following format: `test-parachain-<DATE>-<COMMIT_HASH>`.
    You can see the available `DOCKER_TAG`s [here](https://hub.docker.com/r/centrifugeio/centrifuge-chain/tags)

    Open **Centrifuge Local** client [here](https://polkadot.js.org/apps/?rpc=ws%3A%2F%2Flocalhost%3A9946#/explorer)

1. After see block production in the **relay chain^**, run the boarding:
    ```sh
    DOCKER_ONBOARD=true PARA_DOCKER_IMAGE_TAG=<DOCKER_TAG> PARA_CHAIN_SPEC=centrifuge-local ./scripts/init.sh onboard-parachain
    ```
    After 2 minuts you should see block production in the **parachain**

1. Open the Github **release** section and find the release/runtime you want to test.
    - Copy the `BLAKE2_256` hash
    - Download your runtime assert `.wasm` file.

1. In **Centrifuge Local** client -> Governance -> Democracy, click on **send preimage**.
    - At **proponer**, choose: `parachainSystem` with `authorizeUpgrade`
    - At **codeHash**, copy the `BLAKE2_256` of the previous step.
    - Click on **send preimage**

1. In Network -> Explorer -> Check the `democracy.PreImageNoted` event where the preimage of your blake hash is dispatched
    and copy it.

1. In Governance -> Council -> Motions, click on **propose a motion**.
    - At **umbral**, put `3` (a 75% of the total which is 4).
    - At **propose**: choose: `utility` with `batch` in order to propose a motion with several calls.
        - First call, choose: `democracy` with `externalProposedMajority` and add the copied preimage value as **proposedHash**.
        - Second call, choose: `democracy` with `fastTract` and add the copied preimage value as **proposedHash** and
            write a `10` on **votingPeriod**.

    - Click on **propose**

1. Voting phase. You should make 3 votes in favor with 3 diferent people (modify the CFG amount before to a low number).
    **Warning**, once the proposal appears, you will have `10 * 12` (`votingPeriod * secons_per_block`) seconds to perform
    all votes in favor.

    Once the time ends, close and sign the votation.

1. In Developer -> Extrinsics, choose `parachainSystem` with `enactAuthorizedUpgrade`.
    Click then on **file upload** and upload the `.wasm` file previously downlaoded.

TODO: check the runtime is done ok.
