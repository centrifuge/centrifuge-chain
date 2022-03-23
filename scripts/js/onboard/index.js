const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api');
const fs = require('fs')

const run = async () => {
  try {
    console.log("Parsing Args ...")
    // 0 & 1 are command context
    const endpoint = process.argv[2];
    const seed = process.argv[3];
    const id = process.argv[4];
    const header = process.argv[5];
    const wasmFile = process.argv[6]

    const wsProvider = new WsProvider(endpoint);

    const api = await ApiPromise.create({
      provider: wsProvider,
    });

    const keyring = new Keyring({ type: "sr25519" });
    const alice = keyring.addFromUri(seed);

    let wasm;
    try {
      wasm = fs.readFileSync(wasmFile, 'utf8')
    } catch (err) {
      console.error(err)
      throw err
    }

    let paraGenesisArgs = {
      genesis_head: header,
      validation_code: wasm,
      parachain: true,
    };
    let genesis = api.createType("ParaGenesisArgs", paraGenesisArgs);

    const nonce = Number((await api.query.system.account(alice.address)).nonce);

    console.log(
        `--- Submitting extrinsic to register parachain ${id}. (nonce: ${nonce}) ---`
    );
    const unsub = await api.tx.sudo
        .sudo(api.tx.parasSudoWrapper.sudoScheduleParaInitialize(id, genesis))
        .signAndSend(alice, { nonce: nonce, era: 0 }, (result) => {
          console.log(`Current status is ${result.status}`);
          if (result.status.isInBlock) {
            console.log(
                `Transaction included at blockHash ${result.status.asInBlock}`
            );
            console.log("Waiting for finalization...");
          } else if (result.status.isFinalized) {
            console.log(
                `Transaction finalized at blockHash ${result.status.asFinalized}`
            );
            unsub();
            process.exit()
          } else if (result.isError) {
            console.log(`Transaction Error`);
            process.exit()
          }
        });

  } catch (error) {
    console.log('error:', error);
  }

};

run();
