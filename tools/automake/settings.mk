###############################################################################
# Centrifuge                                                                  #
# Cash on Steroids                                                            #
#                                                                             #
# tools/automake/settings.mk                                                  #
#                                                                             #
# Handcrafted since 2020 by Centrifugians                                     #
# All rights reserved                                                         #
#                                                                             #
#                                                                             #
# Description: Project's configuration, including, for instance, the version  #
#              of the Rust toolchain, the Docker image name of the developer' #
#              sandbox, and so on.                                            #
###############################################################################


# Sandbox Docker image name
SANDBOX_DOCKER_IMAGE_NAME=centrifuge/sandbox

# Sandbox Docker iamge tag (should be better based on a Git hash)
SANDBOX_DOCKER_IMAGE_TAG=latest

# Rust compiler version to use in the developer' sandbox
RUST_VERSION=1.51.0

# Rustup tool version to use in the developer' sandbox
RUSTUP_VERSION=1.23.1

# Default Rust toolchain (depends on Substrate's version) to use in developer' sandbox
RUST_TOOLCHAIN=nightly-2020-08-16

# Sandbox container configuration (should be tweaked according to host machine)
SANDBOX_CONFIG_MEMORY_SIZE=10GB
SANDBOX_CONFIG_SWAP_SIZE=2GB
SANDBOX_CONFIG_CPUS=2
