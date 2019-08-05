#!/usr/bin/env bash

docker build -t centrifugeio/centrifuge-chain .
echo "$DOCKER_PASSWORD" | docker login -u "$DOCKER_USERNAME" --password-stdin
docker push centrifugeio/centrifuge-chain