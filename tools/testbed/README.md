# Centrifuge Chain Testbed

## Introduction
This testbed environment creates a containerized infrastructure to test the Centrifuge chain, as it is currently used to operate the current [Centrifuge's mainnet](https://portal.chain.centrifuge.io/#/explorer). 

It relies on [Parity Substrate API Sidecar](https://github.com/paritytech/substrate-api-sidecar) and [`Parity txwrapper-core`](https://github.com/paritytech/txwrapper-core) tools.

## Requisite Tools
Only the following tools must be installed on your host in order to play with this testbed:
- [Docker](https://docs.docker.com/get-docker/)
- [Docker Compose](https://docs.docker.com/compose/install/)
- [Make tools](https://www.gnu.org/software/make/)
## Getting Started

In order to land and work with this testbed environment, please execute the following command in a terminal so that to create the necessary Docker images (be patient folks, it takes time):

```sh
$ make setup
```
This seminal setup command must be run only once, or after you clean up the testbed environment with `make clean`
command.

You are now ready to launch the testbed infrastructure, that is made up of a container with substrate API sidecar (called `testbed-sidecar`) and another (called `testbed-chain`) with the Centrifuge chain's mainnet. For doing so, enter the following command:

```sh
$ make start
```

You are now ready to run transactions (or extrinsics) on the Centrifuge chain using Substrate API service. 

## Tweak Parameters

All parameters are declared in the [`settings.mk`](./automake/settings.mk) file.

See also [`docker-compose-chain.yml`](./docker/docker-compose-chain.yml) and [`docker-compose-sidecar.yml`](./docker/docker-compose-sidecar.yml).

## Testing Scenari

Testing cases can be implemented in Javascript/Typescript, using [`Parity txwrapper-core`](https://github.com/paritytech/txwrapper-core) tools.

Some simple tests will soon be available in the [`tests`](./tests) folder.

## References

[Parity Substrate API Sidecar](https://github.com/paritytech/substrate-api-sidecar)
[Parity `txwrapper-core`](https://github.com/paritytech/txwrapper-core)
[Centrifuge Chain](https://github.com/centrifuge/centrifuge-chain/tree/master)