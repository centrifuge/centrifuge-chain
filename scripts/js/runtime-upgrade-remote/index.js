const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api');
const { u8aToHex } = require('@polkadot/util');
const { blake2AsHex, blake2AsU8a } = require('@polkadot/util-crypto');
const fs = require('fs');
const path = require('path');

// Load configuration
const configPath = path.resolve(__dirname, 'config.json');
const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));

const run = async () => {
  let exitCode = 0;
  try {
    // Validate configuration
    if (!config.endpoint || !config.wasmFile || !config.privateKey) {
      console.error("Missing configuration parameters. Please ensure 'endpoint', 'wasmFile', and 'privateKey' are specified in the corresponding configs/*.json.");
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
    const wasmHash = blake2AsHex(wasm);
    const wasmBytes = u8aToHex(wasm);

    console.log("WASM file loaded and ready for deployment");

    if (config.sudo) {
      console.log("Using sudo to perform the runtime upgrade");
      await sudoAuthorize(api, user, wasmHash);
      await enactUpgrade(api, user, wasmBytes);
    } else {
      console.error("Unsupported");
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
  const nonce = await api.rpc.system.accountNextIndex(sudoAccount.address)

  return new Promise(async (resolve, reject) => {
    try {
      // Authorize the upgrade
      const authorizeTx = api.tx.sudo.sudo(
        api.tx.parachainSystem.authorizeUpgrade(wasmHex, true)
      );

      const unsub = await authorizeTx.signAndSend(sudoAccount, { nonce }, ({ status, dispatchError, events }) => {
        console.log(`Authorizing upgrade with status ${status}`);
        if (status.isInBlock) {
          console.log(`Authorization included in block ${status.asInBlock}`);
          resolve();
          unsub();
        }
        checkError(api, reject, dispatchError, events)
      });
    }
    catch (error) {
      reject(error)
    }
  });
}

async function enactUpgrade(api, sudoAccount, wasmFile) {
  const nonce = await api.rpc.system.accountNextIndex(sudoAccount.address)

  return new Promise(async (resolve, reject) => {
    try {
      // Enact the authorized upgrade
      const enactTx = api.tx.parachainSystem.enactAuthorizedUpgrade(wasmFile);

      const unsub = await enactTx.signAndSend(sudoAccount, { nonce }, ({ status, dispatchError, events }) => {
        console.log(`Enacting upgrade with status ${status}`);
        if (status.isInBlock) {
          console.log(`Enactment included in block ${status}`);
          resolve();
          unsub();
        }
        checkError(api, reject, dispatchError, events)
      });
    }
    catch (error) {
      reject(error)
    }
  });
}

function checkError(api, reject, dispatchError, events) {
  if (dispatchError) {
    if (dispatchError.isModule) {
      // for module errors, we have the section indexed, lookup
      const decoded = api.registry.findMetaError(dispatchError.asModule);
      const { docs, name, section } = decoded;

      console.error(`${section}.${name}: ${docs.join(' ')}`);
    } else {
      // Other, CannotLookup, BadOrigin, no extra info
      console.error(dispatchError.toString());
    }
    reject(dispatchError)
  } else if (events) {
    events
      // find/filter for failed events
      .filter(({ event }) =>
        api.events.system.ExtrinsicFailed.is(event)
      )
      // we know that data for system.ExtrinsicFailed is
      // (DispatchError, DispatchInfo)
      .forEach(({ event: { data: [error, info] } }) => {
        if (error.isModule) {
          // for module errors, we have the section indexed, lookup
          const decoded = api.registry.findMetaError(error.asModule);
          const { docs, method, section } = decoded;
          const error = `${section}.${method}: ${docs.join(' ')}`

          console.error(error);
          reject(error)
        } else {
          // Other, CannotLookup, BadOrigin, no extra info
          console.error(error.toString());
          reject(error.toString())
        }
      });
  }
}

run();