#!/usr/bin/env bash

set -e

NODE_NAME=$1

echo "*** Running Centrifuge Chain Node for [$1]"

case NODE_NAME in
	"Alice")
        ././../target/release/centrifuge-chain --dev --ws-external --validator --chain=local --node-key=2a654a0958cd0e10626c36057c46a08018eaf2901f9bab74ecc1144f714300ac --bootnodes=/ip4/127.0.0.1/tcp/30334/p2p/QmSqbcHcJh7DvKDdMYxWREtnAfqqxLiX7J2YDGiV6e5LQq --port=30333 --rpc-port=9933 --ws-port=9944 --name=Alice --key=//Alice -d ~/tmp/centrifuge-chain/alice
		;;

	"Babette")
        ././../target/release/centrifuge-chain --dev --ws-external --validator --chain=local --node-key=66ef62065cfdc48929b5cb9c1bbc0a728e6d1d43b4ba1de13ccf76c7ecec66e9 --bootnodes=/ip4/127.0.0.1/tcp/30333/p2p/QmctF8dCW8LBr6zqVEUJHmjmqFcsxjV91tuUL7rVLg3Zd6 --port=30334 --rpc-port=9934 --ws-port=9945 --name=Babette --key=//Bob -d ~/tmp/centrifuge-chain/babette
		;;

	"")
		echo "Please supply name of node to run. Either Alice or Babette."
		;;
esac