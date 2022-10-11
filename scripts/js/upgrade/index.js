const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api');
const { blake2AsHex } = require('@polkadot/util-crypto');
const fs = require('fs')
const util = require('util');
const exec = util.promisify(require('child_process').exec);

const run = async () => {
  try {
    console.log("Parsing Args ...")
    // 0 & 1 are command context
    const endpoint = "ws://0.0.0.0:9946";
    const endpointRelay = "ws://0.0.0.0:9944";
    const seeds = ["//Alice", "//Bob", "//Charlie"];
    const wasmFile = process.argv[2];
    const targetDockerTag = process.argv[3];

    const wsProvider = new WsProvider(endpoint);
    const wsProviderRelay = new WsProvider(endpointRelay);

    console.log("Starting Relay Chain and waiting until is up")
    await execCommand('cd ../../../ && ./scripts/init.sh start-relay-chain')

    const apiRelay = await ApiPromise.create({
      provider: wsProviderRelay,
    });

    console.log("Waiting until relay chain is producing blocks")
    await waitUntilEventFound(apiRelay, "ExtrinsicSuccess")

    console.log("Starting Centrifuge Chain and waiting until is up")
    process.env.CC_DOCKER_TAG = targetDockerTag
    process.env.PARA_CHAIN_SPEC = "centrifuge-local" // Make it configurable
    await execCommand('cd ../../../ && ./scripts/init.sh start-parachain-docker')

    const api = await ApiPromise.create({
      provider: wsProvider,
    });

    console.log("Centrifuge Chain started, onboarding parachain now")
    process.env.DOCKER_ONBOARD = true
    process.env.PARA_DOCKER_IMAGE_TAG = targetDockerTag
    await execCommand('cd ../../../ && ./scripts/init.sh onboard-parachain')

    console.log("Waiting until Centrifuge Chain is producing blocks")
    await waitUntilEventFound(api, "ExtrinsicSuccess")

    const keyring = new Keyring({ type: "sr25519" });
    const alice = keyring.addFromUri(seeds[0]);
    const bob = keyring.addFromUri(seeds[1]);
    const charlie = keyring.addFromUri(seeds[2]);

    const wasm = fs.readFileSync(wasmFile)
    const wasmHash = blake2AsHex(wasm)
    console.log("Applying WASM Blake2 Hash:", wasmHash)

    nonce = Number((await api.query.system.account(alice.address)).nonce);
    const preimageNoted = await notePreimageAuth(api, alice, wasmHash, nonce);

    console.log("Continuing with council proposal using", preimageNoted)
    nonce = Number((await api.query.system.account(alice.address)).nonce);
    const result = await councilProposeDemocracy(api, alice, preimageNoted, nonce)

    console.log("Continuing with council vote using", result[0], result[1])
    nonce = Number((await api.query.system.account(alice.address)).nonce);
    await councilVoteProposal(api, alice, result[0], result[1], nonce, false)
    nonce = Number((await api.query.system.account(bob.address)).nonce);
    await councilVoteProposal(api, bob, result[0], result[1], nonce, false)
    nonce = Number((await api.query.system.account(charlie.address)).nonce);
    await councilVoteProposal(api, charlie, result[0], result[1], nonce, true)

    console.log("Continuing to close council vote")
    nonce = Number((await api.query.system.account(alice.address)).nonce);
    const democracyIndex = await councilCloseProposal(api, alice, result[0], result[1], nonce)

    console.log("Continuing with democracy vote on ref index", democracyIndex)
    nonce = Number((await api.query.system.account(alice.address)).nonce);
    await voteReferenda(api, alice, democracyIndex, nonce)

    console.log("Waiting for referenda to be over and UpgradeAuthorized event is triggered")
    await waitUntilEventFound(api, "UpgradeAuthorized")

    console.log("Proceeding to enact upgrade")
    let nonce = Number((await api.query.system.account(alice.address)).nonce);
    await enactUpgrade(api, alice, `0x${wasm.toString('hex')}`, nonce);

    console.log("Waiting for ValidationFunctionApplied event")
    await waitUntilEventFound(api, "ValidationFunctionApplied")

    console.log("Waiting for 3 NewSession events")
    await waitUntilEventFound(api, "EmptyTerm")
    // await waitUntilEventFound(api, "NewSession")

    console.log("First event found, waiting for second event")
    await waitUntilEventFound(api, "EmptyTerm")
    // await waitUntilEventFound(api, "NewSession")

    console.log("Second event found, waiting for third event")
    await waitUntilEventFound(api, "EmptyTerm")
    // await waitUntilEventFound(api, "NewSession")

    console.log("Runtime Upgrade succeeded")

    process.exit(0)
  } catch (error) {
    console.log('error:', error);
    process.exit(1);
  } finally {
    await execCommand('cd ../../../ && ./scripts/init.sh stop-parachain-docker')
    await execCommand('cd ../../../ && ./scripts/init.sh stop-relay-chain')
  }

};

async function execCommand(strCommand) {
  try {
    const { stdout, stderr } = await exec(strCommand);
    console.log('stdout:', stdout);
    console.log('stderr:', stderr);
  } catch (err) {
    console.error(err);
  }
}

async function waitUntilEventFound(api, eventName) {
  return new Promise(async (resolve, reject) => {
    let maxCountDown = 30;
    const unsubscribe = await api.rpc.chain.subscribeNewHeads(async (header) => {
      maxCountDown--
      if (maxCountDown === 0) {
        unsubscribe()
        reject(`Timeout waiting for event ${eventName}`)
      }
      console.log(`Chain is at block: #${header.number} & hash: ${header.hash}`);
      const at = await api.at(header.hash);
      const events = await at.query.system.events();
      events.forEach((er) => {
        console.log("EvName", er.event.method)
        if (er.event.method === eventName) {
          unsubscribe()
          resolve()
        }
      })

    });
  });
}

async function notePreimageAuth(api, alice, wasmFileHash, nonce) {
  return new Promise((resolve, reject) => {
    let authorizeUpgradeCall = api.tx.parachainSystem.authorizeUpgrade(wasmFileHash)

    let preimageNoted = "";
    console.log(
        `--- Submitting extrinsic to notePreimage ${wasmFileHash}. (nonce: ${nonce}) ---`
    );
    api.tx.democracy.notePreimage(authorizeUpgradeCall.method.toHex())
        .signAndSend(alice, {nonce: nonce, era: 0}, (result) => {
          console.log(`Current status is ${result.status}`);
          if (result.status.isInBlock) {
            console.log(
                `Transaction included at blockHash ${result.status.asInBlock}`
            );
            result.events.forEach((er) => {
              if (er.event.method === "PreimageNoted") {
                preimageNoted = er.event.data[0].toHex()
              }
            })
            console.log("PreimageNoted", preimageNoted);
            resolve(preimageNoted)
          } else if (result.isError) {
            let error = result.asError;
            console.log("AsError", error);
            // if (error.isModule) {
            //   // for module errors, we have the section indexed, lookup
            //   const decoded = api.registry.findMetaError(error.asModule);
            //   const { docs, name, section } = decoded;
            //
            //   console.log(`${section}.${name}: ${docs.join(' ')}`);
            // } else {
            //   // Other, CannotLookup, BadOrigin, no extra info
            //   console.log(error.toString());
            // }
            console.log(`Transaction Error: ${result.dispatchError}`);
            reject("blabla bad")
          }
        });
  });
}

async function councilProposeDemocracy(api, alice, preimageHash, nonce) {
  return new Promise((resolve, reject) => {
    const txs = [
      api.tx.democracy.externalProposeMajority(preimageHash),
      api.tx.democracy.fastTrack(preimageHash, 10, 0)
    ];

    let batchAllDemocracy = api.tx.utility.batchAll(txs)

    console.log(
        `--- Submitting extrinsic to propose preimage to council. (nonce: ${nonce}) ---`
    );
    api.tx.council.propose(3, batchAllDemocracy, 82)
        .signAndSend(alice, {nonce: nonce, era: 0}, (result) => {
          console.log(`Current status is ${result.status}`);
          if (result.status.isInBlock) {
            console.log(
                `Transaction included at blockHash ${result.status.asInBlock}`
            );
            let councilProposalHash = "";
            let councilProposalIndex = "";
            result.events.forEach((er) => {
              if (er.event.method === "Proposed") {
                councilProposalIndex = er.event.data[1]
                councilProposalHash = er.event.data[2].toHex()
              }
            })
            console.log("CouncilProposalHashAndIndex", councilProposalHash, councilProposalIndex);
            resolve([councilProposalHash, councilProposalIndex])
          } else if (result.isError) {
            console.log(`Transaction Error`);
            reject("blabla bad")
          }
        });
  });
}

async function councilVoteProposal(api, account, proposalHash, proposalIndex, nonce, wait) {
  return new Promise((resolve, reject) => {
    console.log(
        `--- Submitting extrinsic to vote on council motion. (nonce: ${nonce}) ---`
    );
    api.tx.council.vote(proposalHash, proposalIndex, true)
        .signAndSend(account, {nonce: nonce, era: 0}, (result) => {
          console.log(`Current status is ${result.status}`);
          if (!wait) {
            resolve()
          }
          if (result.status.isInBlock) {
            console.log(
                `Transaction included at blockHash ${result.status.asInBlock}`
            );
            resolve()
          } else if (result.isError) {
            console.log(`Transaction Error`);
            reject("blabla bad")
          }
        });
  });
}

async function councilCloseProposal(api, account, proposalHash, proposalIndex, nonce) {
  return new Promise((resolve, reject) => {
    console.log(
        `--- Submitting extrinsic to close council motion. (nonce: ${nonce}) ---`
    );

    api.tx.council.close(proposalHash, proposalIndex, 52865600000, 82)
        .signAndSend(account, {nonce: nonce, era: 0}, (result) => {
          console.log(`Current status is ${result.status}`);
          if (result.status.isInBlock) {

            console.log(
                `Transaction included at blockHash ${result.status.asInBlock}`
            );

            let democracyIndex;
            result.events.forEach((er) => {
              if (er.event.method === "Started") {
                democracyIndex = er.event.data[0]
              }
            })

            resolve(democracyIndex)
          } else if (result.isError) {
            console.log(`Transaction Error`);
            reject("blabla bad")
          }
        });
  });
}

async function voteReferenda(api, account, refIndex, nonce) {
  return new Promise((resolve, reject) => {
    console.log(
        `--- Submitting extrinsic to vote on referenda. (nonce: ${nonce}) ---`
    );

    let vote = {
      Standard: {
        vote: {
          aye: true,
          conviction: 0
        },
        balance: "1000000000000000000"
      }
    }

    api.tx.democracy.vote(refIndex, vote)
        .signAndSend(account, {nonce: nonce, era: 0}, (result) => {
          console.log(`Current status is ${result.status}`);
          if (result.status.isInBlock) {
            console.log(
                `Transaction included at blockHash ${result.status.asInBlock}`
            );
            resolve()
          } else if (result.isError) {
            console.log(`Transaction Error`);
            reject("blabla bad")
          }
        });
  });
}

async function enactUpgrade(api, account, wasmCode, nonce) {
  return new Promise((resolve, reject) => {
    console.log(
        `--- Submitting extrinsic to enact upgrade. (nonce: ${nonce}) ---`
    );

    api.tx.parachainSystem.enactAuthorizedUpgrade(wasmCode)
        .signAndSend(account, {nonce: nonce, era: 0}, (result) => {
          console.log(`Current status is ${result.status}`);
          if (result.status.isInBlock) {
            console.log(
                `Transaction included at blockHash ${result.status.asInBlock}`
            );
            resolve()
          } else if (result.isError) {
            console.log(`Transaction Error`);
            reject("blabla bad")
          }
        });
  });
}

run();
