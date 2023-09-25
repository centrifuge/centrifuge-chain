# Testing a runtime upgrade on a local environment
This must be done before releasing a new version of any of our runtimes to ensure that the chain still produces blocks once the latest version is applied.

1. Run the relay chain:
    ```sh
    ./scripts/init.sh start-relay-chain
    ```
    Open **Rococo Local Testnet** client [here](https://polkadot.js.org/apps/?rpc=ws%3A%2F%2F127.0.0.1%3A9944#/explorer)

1. Run the parachain using the last available release:
    ```sh
    PARA_CHAIN_SPEC=centrifuge-local CC_DOCKER_TAG=<DOCKER_TAG> ./scripts/init.sh start-parachain-docker
    ```
    The `DOCKER_TAG` has the following format: `test-main-<DATE>-<COMMIT_HASH>`.
    You can see the available `DOCKER_TAG`s [here](https://hub.docker.com/r/centrifugeio/centrifuge-chain/tags)

    Open the **Centrifuge Local** client [here](https://polkadot.js.org/apps/?rpc=ws%3A%2F%2Flocalhost%3A9946#/explorer)

1. After verifying block production in the **relay chain**, onboard the parachain:
    ```sh
    DOCKER_ONBOARD=true PARA_DOCKER_IMAGE_TAG=<DOCKER_TAG> PARA_CHAIN_SPEC=centrifuge-local ./scripts/init.sh onboard-parachain
    ```
    After 2 minutes you should see block production in the **parachain**

1. Open the Github **release** section and find the release/runtime you want to test.
    - Copy the `BLAKE2_256` hash
    - Download your runtime assert `.wasm` file.

1. In **Centrifuge Local** client -> Governance -> Democracy, click on **Submit preimage**.
    - At **proposer**, choose: `parachainSystem` with `authorizeUpgrade`
    - At **codeHash**, copy the `BLAKE2_256` of the previous step.
    - Click on **Submit preimage**

1. In Network -> Explorer -> Check the `democracy.PreImageNoted` event where the preimage of your blake hash is dispatched
    and copy it.

1. In Governance -> Council -> Motions, click on **propose motion**.
    - At **umbral**, put `3` (a 75% of the total which is 4).
    - At **propose**: choose: `utility` with `batch` in order to propose a motion with several calls.
        - First call, choose: `democracy` with `externalProposedMajority` and add the copied preimage value as **proposedHash**.
        - Second call, choose: `democracy` with `fastTrack` and add the copied preimage value as **proposedHash** and
            write a `10` on **votingPeriod**.

    - Click on **propose**

1. Voting phase. You should make 3 votes in favor of 3 different people (modify the CFG amount before to a low number).
    **Warning**, once the proposal appears, you will have `10 * 12` (`votingPeriod * secons_per_block`) seconds to perform
    all votes in favor.

    Once the time ends, **close** the motion.

1. Go to democracy and add a vote to the referendum.
    (You can reduce the amount used to vote if the account doesn't have enough).
    **Warning**, you will have a reduced time to make this step.

1. In Developer -> Extrinsics, choose `parachainSystem` with `enactAuthorizedUpgrade`.
    Click then on **file upload** and upload the `.wasm` file previously downloaded.
