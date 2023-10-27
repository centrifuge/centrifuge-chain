# Runtime Generic tests

The aim of this module is to make integration-tests independently for all runtimes at once.

You can choose the environment for each of your use cases:
  - `RuntimeEnv`: Simple environment that acts as a wrapper over the runtime
  - `FudgeEnv`: Advanced environment that use a client and connect the runtime to a relay chain.

Both environment uses the same interface so jumping from one to the another should be something "smooth".

## Where I start?
- Create a new file in `cases/<file.rs>` for the use case you want to test.
- Maybe you need to update the `Runtime` trait in `config.rs` file with extra information from your new pallet.
  This could imply:
    - Adding bounds to the `Runtime` trait with your new pallet.
    - Adding bounds to `T::RuntimeCallExt` to support calls from your pallet.
    - Adding bounds to `T::EventExt` to support events from your pallet.
    - Adding bounds to `T::Api` to support new api calls.
- You can add `GenesisBuild` builders for setting the initial state of your pallet for others in `utils/genesis.rs`.
  Please be as generic and simple as possible to leave others to compose its own requirement using your method,
  without hidden initializations.
- You can add any utility that helps to initialize states for others under `utils` folder.
  Again, focus in simplity but without side effects or hidden / non-obvious state changes.
