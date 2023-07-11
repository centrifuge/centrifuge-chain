const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api');
const { blake2AsHex } = require('@polkadot/util-crypto');
const fs = require('fs')
const util = require('util');
const exec = util.promisify(require('child_process').exec);

// Needs to be >= 34
// 32 bytes from the encoding of the H256 hashed WASM blob
// 2 for pallet and extrinsic indices
const AUTHORIZE_UPGRADE_PREIMAGE_BYTES = 34;
// Needs to be >= 84
// 39 from democracy.externalProposeMajority(Lookup(H256, 34)))
// 42 from democracy.fastTrack(H256, ...)
// 1 from remark.batchAll
// 2 for pallet and extrinsic indices
const COUNCIL_PROPOSAL_BYTES = 90;
// arbitrary numbers
const FAST_TRACK_VOTE_BLOCKS = 15;
const FAST_TRACK_DELAY_BLOCKS = 0;
const MAX_COUNT_DOWN_BLOCKS = 30;
const POST_UPGRADE_WAITING_SESSIONS = 3;

const run = async () => {
  let exitCode = 0;
  try {
    const endpoint = "ws://0.0.0.0:9946";
    const endpointRelay = "ws://0.0.0.0:9944";
    const ALICE = "//Alice";
    const BOB = "//Bob";
    const CHARLIE = "//Charlie";

    // input args: 0 & 1 are command context
    console.log("Parsing Args ...")
    const wasmFile = process.argv[2];
    const targetDockerTag = process.argv[3];
    const chainSpec = process.argv[4] !== undefined ? process.argv[4] : 'centrifuge-local';

    console.log("Starting Relay Chain and waiting until is up")
    await execCommand('cd ../../../ && ./scripts/init.sh start-relay-chain')

    const wsProviderRelay = new WsProvider(endpointRelay);
    const apiRelay = await ApiPromise.create({
      provider: wsProviderRelay,
    });

    console.log("Waiting until relay chain is producing blocks")
    await waitUntilEventFound(apiRelay, "ExtrinsicSuccess")

    console.log("Starting Centrifuge Chain and waiting until is up")
    process.env.CC_DOCKER_TAG = targetDockerTag
    process.env.PARA_CHAIN_SPEC = chainSpec
    await execCommand('cd ../../../ && ./scripts/init.sh start-parachain-docker')

    const wsProvider = new WsProvider(endpoint);
    const api = await ApiPromise.create({
      provider: wsProvider,
    });

    console.log("Centrifuge Chain started, onboarding parachain now")
    process.env.DOCKER_ONBOARD = true
    process.env.PARA_DOCKER_IMAGE_TAG = targetDockerTag
    await execCommand('cd ../../../ && ./scripts/init.sh onboard-parachain')

    console.log("Waiting until Centrifuge Chain is producing blocks")
    await waitUntilEventFound(api, "ExtrinsicSuccess")

    // Wait one extra session due to facing random errors when starting to send txs too close to the onboarding step
    // await waitUntilEventFound(api, "NewSession")
    await waitUntilEventFound(api, "NewSession") //TODO: Change to "NewSession" once we have built a runtime upgrade with a version increment

    const keyring = new Keyring({ type: "sr25519" });
    const alice = keyring.addFromUri(ALICE);
    const bob = keyring.addFromUri(BOB);
    const charlie = keyring.addFromUri(CHARLIE);

    const wasm = fs.readFileSync(wasmFile)
    const wasmHash = blake2AsHex(wasm)
    console.log("Applying WASM Blake2 Hash:", wasmHash)

    let nonce = await getNonce(api, alice.address);
    const preimageNoted = await notePreimageAuth(api, alice, wasmHash, nonce);

    console.log("Continuing with council proposal using", preimageNoted)
    nonce = await getNonce(api, alice.address);
    const result = await councilProposeDemocracy(api, alice, preimageNoted, nonce)

    console.log("Continuing with council vote using", result[0], result[1])
    nonce = await getNonce(api, alice.address);
    await councilVoteProposal(api, alice, result[0], result[1], nonce, false)
    nonce = await getNonce(api, bob.address);
    await councilVoteProposal(api, bob, result[0], result[1], nonce, false)
    nonce = await getNonce(api, charlie.address);
    await councilVoteProposal(api, charlie, result[0], result[1], nonce, true)

    console.log("Continuing to close council vote")
    nonce = await getNonce(api, alice.address);
    const democracyIndex = await councilCloseProposal(api, alice, result[0], result[1], nonce)

    console.log("Continuing with democracy vote on ref index", democracyIndex)
    nonce = await getNonce(api, alice.address);
    await voteReferenda(api, alice, democracyIndex, nonce)

    console.log("Waiting for referenda to be over and UpgradeAuthorized event is triggered")
    await waitUntilEventFound(api, "UpgradeAuthorized")

    console.log("Proceeding to enact upgrade")
    nonce = await getNonce(api, alice.address);
    await enactUpgrade(api, alice, `0x${wasm.toString('hex')}`, nonce);

    console.log("Waiting for ValidationFunctionApplied event")
    await waitUntilEventFound(api, "ValidationFunctionApplied")

    console.log(`Waiting for ${POST_UPGRADE_WAITING_SESSIONS} NewSession events`)
    let foundInBlock = 0;
    for (let i = 0; i < POST_UPGRADE_WAITING_SESSIONS; i++) {
      foundInBlock = await waitUntilEventFound(api, "NewSession", foundInBlock + 1)
      console.log(`Session ${i + 1}/${POST_UPGRADE_WAITING_SESSIONS}`)
    }

    console.log("Runtime Upgrade succeeded")

  } catch (error) {
    console.log('error:', error);
    exitCode = 1;
  } finally {
    await execCommand('cd ../../../ && ./scripts/init.sh stop-parachain-docker')
    await execCommand('cd ../../../ && ./scripts/init.sh stop-relay-chain')
    process.exit(exitCode)
  }

};

async function getNonce(api, address) {
  return Number((await api.query.system.account(address)).nonce)
}

async function execCommand(strCommand) {
  try {
    const { stdout, stderr } = await exec(strCommand);
    console.log('stdout:', stdout);
    console.log('stderr:', stderr);
  } catch (err) {
    console.error(err);
  }
}

async function waitUntilEventFound(api, eventName, fromBlock = 0) {
  return new Promise(async (resolve, reject) => {
    let maxCountDownBlocks = MAX_COUNT_DOWN_BLOCKS;
    const unsubscribe = await api.rpc.chain.subscribeNewHeads(async (header) => {
      maxCountDownBlocks--
      if (maxCountDownBlocks === 0) {
        unsubscribe()
        reject(`Timeout waiting for event ${eventName}`)
      }
      console.log(`Chain is at block: #${header.number} & hash: ${header.hash}`);
      const at = await api.at(header.hash);
      const events = await at.query.system.events();
      events.forEach((er) => {
        if ((er.event.method === eventName) && (Number(header.number) > fromBlock)) {
          unsubscribe()
          resolve(Number(header.number))
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
    api.tx.preimage.notePreimage(authorizeUpgradeCall.method.toHex())
      .signAndSend(alice, { nonce: nonce, era: 0 }, (result) => {
        console.log(`Current status is ${result.status}`);
        if (result.status.isInBlock) {
          console.log(
            `Transaction included at blockHash ${result.status.asInBlock}`
          );
          result.events.forEach((er) => {
            if (er.event.method === "Noted") {
              preimageNoted = er.event.data[0].toHex()
            }
          })
          console.log("Noted", preimageNoted);
          resolve(preimageNoted)
        } else if (result.dispatchError) {
          if (result.dispatchError.isModule) {
            // for module errors, we have the section indexed, lookup
            const decoded = api.registry.findMetaError(result.dispatchError.asModule);
            const { docs, name, section } = decoded;
            reject(`${section}.${name}: ${docs.join(' ')}`);
          } else {
            // Other, CannotLookup, BadOrigin, no extra info
            reject(result.dispatchError.toString());
          }
        } else if (result.isError) {
          reject(result)
        }
      });
  });
}

async function councilProposeDemocracy(api, alice, preimageHash, nonce) {
  return new Promise((resolve, reject) => {
    const txs = [
      api.tx.democracy.externalProposeMajority({
        Lookup: {
          hash: preimageHash,
          len: AUTHORIZE_UPGRADE_PREIMAGE_BYTES
        }
      }),
      api.tx.democracy.fastTrack(preimageHash, FAST_TRACK_VOTE_BLOCKS, FAST_TRACK_DELAY_BLOCKS)
    ];

    let batchAllDemocracy = api.tx.utility.batchAll(txs)

    console.log(
      `--- Submitting extrinsic to propose preimage to council. (nonce: ${nonce}) ---`
    );
    api.tx.council.propose(3, batchAllDemocracy, COUNCIL_PROPOSAL_BYTES)
      .signAndSend(alice, { nonce: nonce, era: 0 }, (result) => {
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
        } else if (result.dispatchError) {
          if (result.dispatchError.isModule) {
            // for module errors, we have the section indexed, lookup
            const decoded = api.registry.findMetaError(result.dispatchError.asModule);
            const { docs, name, section } = decoded;
            reject(`${section}.${name}: ${docs.join(' ')}`);
          } else {
            // Other, CannotLookup, BadOrigin, no extra info
            reject(result.dispatchError.toString());
          }
        } else if (result.isError) {
          reject(result)
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
      .signAndSend(account, { nonce: nonce, era: 0 }, (result) => {
        console.log(`Current status is ${result.status}`);
        if (!wait) {
          resolve()
        }
        if (result.status.isInBlock) {
          console.log(
            `Transaction included at blockHash ${result.status.asInBlock}`
          );
          resolve()
        } else if (result.dispatchError) {
          if (result.dispatchError.isModule) {
            // for module errors, we have the section indexed, lookup
            const decoded = api.registry.findMetaError(result.dispatchError.asModule);
            const { docs, name, section } = decoded;
            reject(`${section}.${name}: ${docs.join(' ')}`);
          } else {
            // Other, CannotLookup, BadOrigin, no extra info
            reject(result.dispatchError.toString());
          }
        } else if (result.isError) {
          reject(result)
        }
      });
  });
}

async function councilCloseProposal(api, account, proposalHash, proposalIndex, nonce) {
  return new Promise((resolve, reject) => {
    console.log(
      `--- Submitting extrinsic to close council motion. (nonce: ${nonce}) ---`
    );

    api.tx.council.close(proposalHash, proposalIndex, { refTime: 52865600000, proofSize: 0 }, COUNCIL_PROPOSAL_BYTES)
      .signAndSend(account, { nonce: nonce, era: 0 }, (result) => {
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
        } else if (result.dispatchError) {
          if (result.dispatchError.isModule) {
            // for module errors, we have the section indexed, lookup
            const decoded = api.registry.findMetaError(result.dispatchError.asModule);
            const { docs, name, section } = decoded;
            reject(`${section}.${name}: ${docs.join(' ')}`);
          } else {
            // Other, CannotLookup, BadOrigin, no extra info
            reject(result.dispatchError.toString());
          }
        } else if (result.isError) {
          reject(result)
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
      .signAndSend(account, { nonce: nonce, era: 0 }, (result) => {
        console.log(`Current status is ${result.status}`);
        if (result.status.isInBlock) {
          console.log(
            `Transaction included at blockHash ${result.status.asInBlock}`
          );
          resolve()
        } else if (result.dispatchError) {
          if (result.dispatchError.isModule) {
            // for module errors, we have the section indexed, lookup
            const decoded = api.registry.findMetaError(result.dispatchError.asModule);
            const { docs, name, section } = decoded;
            reject(`${section}.${name}: ${docs.join(' ')}`);
          } else {
            // Other, CannotLookup, BadOrigin, no extra info
            reject(result.dispatchError.toString());
          }
        } else if (result.isError) {
          reject(result)
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
      .signAndSend(account, { nonce: nonce, era: 0 }, (result) => {
        console.log(`Current status is ${result.status}`);
        if (result.status.isInBlock) {
          console.log(
            `Transaction included at blockHash ${result.status.asInBlock}`
          );
          resolve()
        } else if (result.dispatchError) {
          if (result.dispatchError.isModule) {
            // for module errors, we have the section indexed, lookup
            const decoded = api.registry.findMetaError(result.dispatchError.asModule);
            const { docs, name, section } = decoded;
            reject(`${section}.${name}: ${docs.join(' ')}`);
          } else {
            // Other, CannotLookup, BadOrigin, no extra info
            reject(result.dispatchError.toString());
          }
        } else if (result.isError) {
          reject(result)
        }
      });
  });
}

run();
