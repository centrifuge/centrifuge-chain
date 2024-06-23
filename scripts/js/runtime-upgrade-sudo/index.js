const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api');
const { blake2AsHex } = require('@polkadot/util-crypto');
const fs = require('fs');
const path = require('path');

// Constants
const AUTHORIZE_UPGRADE_PREIMAGE_BYTES = 34;
const COUNCIL_PROPOSAL_BYTES = 90;
const COUNCIL_CLOSE_PROOF_SIZE = 1126;
const COUNCIL_CLOSE_REF_TIME = 514033761;
const FAST_TRACK_VOTE_BLOCKS = 5;
const FAST_TRACK_DELAY_BLOCKS = 0;
const MAX_COUNT_DOWN_BLOCKS = 30;
const POST_UPGRADE_WAITING_SESSIONS = 5;

// Load configuration
const configPath = path.resolve(__dirname, 'config.json');
const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));

const run = async () => {
  let exitCode = 0;
  try {
    // Validate configuration
    if (!config.endpoint || !config.wasmFile || !config.privateKey) {
      console.error("Missing configuration parameters. Please ensure 'endpoint', 'wasmFile', and 'privateKey' are specified in config.json.");
      process.exit(1);
    }

    console.log("Configuration loaded:", config);

    const wsProvider = new WsProvider(config.endpoint);
    const api = await ApiPromise.create({ provider: wsProvider });

    console.log("Connected to the parachain at:", config.endpoint);

    const keyring = new Keyring({ type: "sr25519" });
    let user;
    if (config.privateKey.startsWith('//')) {
        user = keyring.addFromUri(config.privateKey);
    } else {
        user = keyring.addFromSeed(config.privateKey);
    }

    console.log(`Using account: ${user.address}`);

    const wasm = fs.readFileSync(config.wasmFile);
    const wasmHex = `0x${wasm.toString('hex')}`;

    console.log("WASM file loaded and ready for deployment");

    let nonce = await getNonce(api, user.address);

    if (config.sudo) {
      console.log("Using sudo to perform the runtime upgrade");
      await sudoUpgrade(api, user, wasmHex);
    } else {
      // Load council members
      const councilMembers = config.councilMembers.map(member => {
        if (member.startsWith('//')) {
          return keyring.addFromUri(member);
        } else {
          return keyring.addFromSeed(member);
        }
      });

      console.log("Council members loaded:", councilMembers.map(member => member.address));

      // Implement council-based upgrade logic here
      console.log("Proceeding with council-based runtime upgrade");

      const wasm = fs.readFileSync(config.wasmFile);
      const wasmHash = blake2AsHex(wasm);
      console.log("Applying WASM Blake2 Hash:", wasmHash);

      let nonce = await getNonce(api, user.address);
      const preimageNoted = await notePreimageAuth(api, user, wasmHash, nonce);

      console.log("Continuing with council proposal using", preimageNoted);
      nonce = await getNonce(api, user.address);
      const result = await councilProposeDemocracy(api, user, preimageNoted, nonce);

      console.log("Continuing with council vote using", result[0], result[1]);
      for (const member of councilMembers) {
        nonce = await getNonce(api, member.address);
        await councilVoteProposal(api, member, result[0], result[1], nonce, false);
      }

      console.log("Continuing to close council vote");
      nonce = await getNonce(api, user.address);
      const democracyIndex = await councilCloseProposal(api, user, result[0], result[1], nonce);

      console.log("Continuing with democracy vote on ref index", democracyIndex);
      nonce = await getNonce(api, user.address);
      await voteReferenda(api, user, democracyIndex, nonce);

      console.log("Waiting for referenda to be over and UpgradeAuthorized event is triggered");
      await waitUntilEventFound(api, "UpgradeAuthorized");

      console.log("Proceeding to enact upgrade");
      nonce = await getNonce(api, user.address);
      await enactUpgrade(api, user, `0x${wasm.toString('hex')}`, nonce);

      console.log("Waiting for ValidationFunctionApplied event");
      await waitUntilEventFound(api, "ValidationFunctionApplied");

      console.log(`Waiting for ${POST_UPGRADE_WAITING_SESSIONS} NewSession events`);
      let foundInBlock = 0;
      for (let i = 0; i < POST_UPGRADE_WAITING_SESSIONS; i++) {
        foundInBlock = await waitUntilEventFound(api, "NewSession", foundInBlock + 1);
        console.log(`Session ${i + 1}/${POST_UPGRADE_WAITING_SESSIONS}`);
      }

      console.log("Runtime Upgrade succeeded");
    }

    // Check for specific events or transaction success as needed

  } catch (error) {
    console.error('Error:', error);
    exitCode = 1;
  } finally {
    process.exit(exitCode);
  }
};

async function sudoUpgrade(api, sudoAccount, wasmHex) {
  // Hash the WASM blob
  const wasmHash = blake2AsHex(wasmHex);

  // Authorize the upgrade
  const authorizeTx = api.tx.sudo.sudo(
      api.tx.parachainSystem.authorizeUpgrade(wasmHash, false)
  );

  console.log("Authorizing the upgrade");
  await authorizeTx.signAndSend(sudoAccount, async ({ status }) => {
      if (status.isInBlock) {
          console.log(`Authorization included in block ${status.asInBlock}`);

          // Enact the authorized upgrade
          const enactTx = api.tx.sudo.sudo(
              api.tx.parachainSystem.enactAuthorizedUpgrade(wasmHash)
          );

          console.log("Enacting the upgrade");
          await enactTx.signAndSend(sudoAccount, ({ status }) => {
              if (status.isInBlock) {
                  console.log(`Enactment included in block ${status.asInBlock}`);
              }
          });
      }
  });
}


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

// NOTE: If council closing fails, the proof size probably needs to be updated.
async function councilCloseProposal(api, account, proposalHash, proposalIndex, nonce) {
  return new Promise((resolve, reject) => {
    console.log(
      `--- Submitting extrinsic to close council motion. (nonce: ${nonce}) ---`
    );

    api.tx.council.close(proposalHash, proposalIndex, { refTime: COUNCIL_CLOSE_REF_TIME, proofSize: COUNCIL_CLOSE_PROOF_SIZE }, COUNCIL_PROPOSAL_BYTES)
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