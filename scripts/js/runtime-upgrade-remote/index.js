const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api');
const { u8aToHex } = require('@polkadot/util');
const { blake2AsHex } = require('@polkadot/util-crypto');
const fs = require('fs');
const path = require('path');

// Constants
const AUTHORIZE_UPGRADE_PREIMAGE_BYTES = 34;
const COUNCIL_PROPOSAL_BYTES = 90;
const FAST_TRACK_VOTE_BLOCKS = 5;
const FAST_TRACK_DELAY_BLOCKS = 0;
// const MAX_COUNT_DOWN_BLOCKS = 30;
// const POST_UPGRADE_WAITING_SESSIONS = 5;
// const COUNCIL_CLOSE_PROOF_SIZE = 1126;
// const COUNCIL_CLOSE_REF_TIME = 514033761;

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
    const wasmBytes = u8aToHex(wasm);

    console.log("WASM file loaded and ready for deployment");

    if (config.sudo) {
      console.log("Using sudo to perform the runtime upgrade");
      await sudoAuthorize(api, user, wasmHex);
      await enactUpgrade(api, user, wasmBytes);
    } else {
      console.log("Using council proposal to perform the runtime upgrade");
      await councilUpgrade(config, api, user)
    }
    // Check for specific events or transaction success as needed
  } catch (error) {
    console.error('Error:', error);
    exitCode = 1;
  } finally {
    process.exit(exitCode);
  }
};

async function sudoAuthorize(api, sudoAccount, wasmHex) {
  const nonce = await getNonce(api, user.address);

  return new Promise(async (resolve, reject) => {
    try {
      // Hash the WASM blob
      const wasmHash = blake2AsHex(wasmHex);
      console.log(wasmHash);

      // Authorize the upgrade
      const authorizeTx = api.tx.sudo.sudo(
        api.tx.parachainSystem.authorizeUpgrade(wasmHash, true)
      );

      const unsub = await authorizeTx.signAndSend(sudoAccount, { nonce }, ({ status, dispatchError }) => {
        console.log(`Authorizing upgrade with status ${status}`);
        if (status.isInBlock) {
          console.log(`Authorization included in block ${status.asInBlock}`);
          resolve();
          unsub();
        }
        if (dispatchError) {
          console.error(`Error: ${dispatchError}`);
          reject(dispatchError);
        }
      });
    }
    catch (error) {
      reject(error)
    }
  });
}

async function enactUpgrade(api, sudoAccount, wasmFile) {
  const nonce = await getNonce(api, user.address);

  return new Promise(async (resolve, reject) => {
    try {
      // Enact the authorized upgrade
      const enactTx = api.tx.parachainSystem.enactAuthorizedUpgrade(wasmFile);

      const unsub = await enactTx.signAndSend(sudoAccount, { nonce }, ({ status, dispatchError }) => {
        console.log(`Enacting upgrade with status ${status}`);
        if (status.isInBlock) {
          console.log(`Enactment included in block ${status}`);
          resolve();
          unsub();
        }
        if (dispatchError) {
          console.error(`Error: ${dispatchError}`);
          reject(dispatchError);
        }
      });
    }
    catch (error) {
      reject(error)
    }
  });
}

async function councilUpgrade(config, api, user) {
  // This code should handle enacting upgrades on a production chain
  // Use with caution as it has not been tested yet
  console.log("Proceeding with council-based runtime upgrade");

  const wasm = fs.readFileSync(config.wasmFile);
  const wasmHash = blake2AsHex(wasm);
  console.log("Applying WASM Blake2 Hash:", wasmHash);

  try {
    let nonce = await getNonce(api, user.address);
    const preimageNoted = await notePreimageAuth(api, user, wasmHash, nonce);

    console.log("Continuing with council proposal using", preimageNoted);
    nonce = await getNonce(api, user.address);
    const [councilProposalHash, councilProposalIndex] = await councilProposeDemocracy(api, user, preimageNoted, nonce);

    console.log("Runtime Upgrade proposed. Council proposal hash:", councilProposalHash, "Index:", councilProposalIndex);
    console.log("The proposal process will take over a week. Please monitor the chain for the outcome.");
  } catch (error) {
    console.error("Error during council upgrade process:", error);
    throw error;
  }
}

async function getNonce(api, address) {
  return Number((await api.query.system.account(address)).nonce)
}

async function notePreimageAuth(api, user, wasmFileHash, nonce) {
  return new Promise((resolve, reject) => {
    let authorizeUpgradeCall = api.tx.parachainSystem.authorizeUpgrade(wasmFileHash)

    let preimageNoted = "";
    console.log(
      `--- Submitting extrinsic to notePreimage ${wasmFileHash}. (nonce: ${nonce}) ---`
    );
    api.tx.preimage.notePreimage(authorizeUpgradeCall.method.toHex())
      .signAndSend(user, { nonce: nonce, era: 0 }, (result) => {
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
            const decoded = api.registry.findMetaError(result.dispatchError.asModule);
            const { docs, name, section } = decoded;
            reject(`${section}.${name}: ${docs.join(' ')}`);
          } else {
            reject(result.dispatchError.toString());
          }
        } else if (result.isError) {
          reject(result)
        }
      });
  });
}

async function councilProposeDemocracy(api, user, preimageHash, nonce) {
  return new Promise((resolve, reject) => {
    const proposeTx = api.tx.democracy.externalProposeMajority({
      Lookup: {
        hash: preimageHash,
        len: AUTHORIZE_UPGRADE_PREIMAGE_BYTES
      }
    });

    console.log(
      `--- Submitting extrinsic to propose preimage to council. (nonce: ${nonce}) ---`
    );
    api.tx.council.propose(api.consts.council.proposalThreshold, proposeTx, COUNCIL_PROPOSAL_BYTES)
      .signAndSend(user, { nonce: nonce, era: 0 }, (result) => {
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
            const decoded = api.registry.findMetaError(result.dispatchError.asModule);
            const { docs, name, section } = decoded;
            reject(`${section}.${name}: ${docs.join(' ')}`);
          } else {
            reject(result.dispatchError.toString());
          }
        } else if (result.isError) {
          reject(result)
        }
      });
  });
}

run();