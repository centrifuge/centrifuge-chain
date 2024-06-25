const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api');
const { u8aToHex } = require('@polkadot/util');
const { blake2AsHex } = require('@polkadot/util-crypto');
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

run();