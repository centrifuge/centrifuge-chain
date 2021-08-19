###############################################################################
# Centrifuge Chain                                                            #
# Cash on Steroids                                                            #
#                                                                             #
# tools/testbed/automake/settings.mk                                          #
#                                                                             #
# Handcrafted since 2020 by Centrifuge Foundation                             #
# All rights reserved                                                         #
#                                                                             #
#                                                                             #
# Description: Testbed's configuration, including, for instance, the version  #
#              of the Rust toolchain, the Docker image name of the developer' #
#              sandbox, and so on.                                            #
###############################################################################

# Docker image name for the compilation sandbox
SANDBOX_DOCKER_IMAGE_NAME=centrifuge/testbed-sandbox
SANDBOX_DOCKER_IMAGE_TAG=latest

# Docker image name for Centrifuge chain's node
CHAIN_DOCKER_IMAGE_NAME=centrifuge/testbed-chain
CHAIN_DOCKER_IMAGE_TAG=$(CHAIN_VERSION)

# Name of Docker network to which the sidecar and the Centrifuge containers
# are attached (do not forget to modify 'docker/docker-compose-networks.yml'
# accordingly)
DOCKER_NETWORK_NAME=testbed-network

# Parameters for the sandbox container
CONTAINER_MEMORY_SIZE=10GB
CONTAINER_SWAP_SIZE=2GB
CONTAINER_CPUS=2

# Name of the Centrifuge chain's executable
CHAIN_EXECUTABLE=centrifuge-chain

# Centrifuge chain version
CHAIN_VERSION=v2.0.0-rc6-243

# Parity Substrate API sidecar
SUBSTRATE_API_SIDECAR_VERSION=8.0.3

# Rust toolchain version
RUST_TOOLCHAIN=nightly-2020-08-16
