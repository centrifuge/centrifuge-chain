# XCM Research

This directory contains the artefacts necessary for the research and development
of XCM features in the Centrifuge chain.


## Local environment

### Start

The local environment we set up is composed of:

```
Relay-Chain
    |——> Cfg development-local Parachain (2000)
    |——> Sibling Parachain               (3000)
```

The sibling parachain is just a clone of `development-local` with another para id.

1. Start the relay chain

```
./scripts/init.sh start-relay-chain
```


2. Start the cfg `development-local` parachain (`2000`)

```
RUST_LOG=info,xcm=trace,xcm-executor=trace \
PARA_CHAIN_SPEC="development-local" \
./scripts/init.sh \
start-parachain purge
```

3. Start the sibling parachain (`3000`)

```
RUST_LOG=info,xcm=trace,xcm-executor=trace \
PARA_CHAIN_SPEC="development-local" \
PARA_ID=3000 \
./scripts/init.sh start-parachain purge
```

4. Onboard the `development-local` parachain

   4.1 Get the genesis head by running:

    ```
    PARA_CHAIN_SPEC="development-local" ./scripts/init.sh onboard-parachain
    ```

   4.2 Onboard the parachain through the Relay chain dashboard on polkadot JS

       Remember to pick the `development-local` runtime wasm: `target/release/wbuild/development-runtime/development_runtime.compact.compressed.wasm`

5. Onboard the `sibling` parachain

   5.1 Get the genesis head by running:

    ```
    PARA_CHAIN_SPEC="development-local" PARA_ID=3000 ./scripts/init.sh onboard-parachain
    ```

   5.2 Onboard the parachain through the Relay chain dashboard on polkadot JS

   Here too, we must use the `development-local` runtime wasm: `target/release/wbuild/development-runtime/development_runtime.compact.compressed.wasm`
   Be also sure to pass `3000` as the Parachain Id.

### Configure

Once both parachains are up and running, onboarded, and producing blocks, we need to configure the local environment
in order to make it possible to send XCM messages between the parachains.

#### Open HRMP channels

**In the relay-chain**, go to `Developer` > `sudo` > `paraSudoWrapper` and select the first option to open an hrmp channel.
Hrmp channels are unidirectional. Be sure to open a channel from parachain `2000` to `3000`. When you do,
you should see `dmp` events in both parachains.

#### Force XCM version

From Polkadot v16, we observe an issue where the previous step fails to set the Xcm version that the receiver parachain
operates in, causing XCM messages on the sender side to fail sending.

On the sender parachain, go to `Developer` > `sudo` > `polkadotXcm` > `forceXcmVersion` and submit a call with these params:

- parent: `1`
- interior: `X1`
    - `Parachain`
    - `3000`
- xcmVersion: `2`


### Make a transfer

On the sender parachain, go to `Developer` > `Extrinsics` > `Decode` tab and paste the one of the following transfers:

- Transfer `10 Native` tokens from Alice (on this chain) to Alice on the receiver parachain

```
0x7c0000000040b2bac9e0191e0200000000000001010200e12e0100d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d00f2052a01000000
```

- Transfer `10 Usd` tokens from Alice (on this chain) to Alice on the receiver parachain

```
0x7c0001000040b2bac9e0191e0200000000000001010200e12e0100d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d00f2052a01000000
```

To verify that the `Usd` tokens were moved as expected, verify the `Usd` balance of the sender and the receiver on the
sender and receiver parachains respectively:

- `Developer` > `Chain State` > `OrmlTokens` and be sure to select the right account and `Usd` as the currency param.

## Integration Tests

In `runtime` > `integration-tests` > `xcm_transfers` we have our integrated tests covering the xcm transfers using an
environment identical to the one described previously that you can setup on your machine but emulated instead.
