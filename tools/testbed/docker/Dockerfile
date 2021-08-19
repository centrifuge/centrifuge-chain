
###############################################################################
# Centrifuge Chain                                                            #
# Cash on Steroids                                                            #
#                                                                             #
# tools/docker/Dockerfile                                                     #
#                                                                             #
# Handcrafted since 2020 by Centrifuge Foundation                             #
# All rights reserved                                                         #
#                                                                             #
#                                                                             #
# Description: Centrifuge chain's Docker image definition.                    #
###############################################################################


# -----------------------------------------------------------------------------
# Sandbox (or compilation playground) stage
# -----------------------------------------------------------------------------

FROM rustlang/rust:nightly AS sandbox

ARG ARG_RUST_TOOLCHAIN="nightly-2020-08-16"

RUN set -eux; \
    apt-get -y update; \
    \
    # Install additional packages (i.e. clan, ssl, ...)
    apt-get install -y --no-install-recommends \
		libssl-dev \
        clang \
        lld \
        libclang-dev \
        make \
        cmake \
		git \
        pkg-config \
        curl \
        wget \
        ca-certificates \
        ; \
    # Set a link to Clang
	update-alternatives --install /usr/bin/cc cc /usr/bin/clang 100; \
    # Install Rust toolchain and WASM target
    rustup update $ARG_RUST_TOOLCHAIN; \
    rustup toolchain install $ARG_RUST_TOOLCHAIN; \
    rustup default $ARG_RUST_TOOLCHAIN; \
    rustup component add rustfmt; \
    rustup target add wasm32-unknown-unknown --toolchain $ARG_RUST_TOOLCHAIN; \
    # Create workspace directory (in which Centrifuge chain's code is copied for compilation)
    mkdir /workspace

# Default working directory
WORKDIR /workspace



# -----------------------------------------------------------------------------
# Build Centrifuge chain executable stage
# -----------------------------------------------------------------------------

FROM sandbox AS builder

ARG ARG_PROFILE="release"
ARG ARG_CHAIN_VERSION="v2.0.0-rc6-241-1"

# Download Centrifuge chain's code base (in /workspace directory, the default working directory)
RUN wget -O centrifuge-chain.tar.gz https://github.com/centrifuge/centrifuge-chain/archive/refs/tags/${ARG_CHAIN_VERSION}.tar.gz; \
    tar xvzf centrifuge-chain.tar.gz --strip 1; \
    # Build Centrifuge chain's executable, for the given profile (be patient folks)
    cargo build --$ARG_PROFILE


# -----------------------------------------------------------------------------
# Build Centrifuge chain service stage
# -----------------------------------------------------------------------------

FROM debian:bullseye-slim AS service

ARG ARG_CHAIN_VERSION="v2.0.0-rc6-241-1"
ARG ARG_IMAGE_BUILD_DATE="unset"
ARG ARG_PROFILE="release"

LABEL io.centrifuge.chain.node.maintainer="tribe@centrifuge.io" \
      io.centrifuge.chain.node.image.version="$ARG_CHAIN_VERSION" \
      io.centrifuge.chain.node.image.description="Centrifuge Chain" \
      io.centrifuge.chain.node.image.vendorName="Centrifuge Foundation" \
      io.centrifuge.chain.node.image.buildDate="$ARG_IMAGE_BUILD_DATE" 

# Install extra runtime dependencies
#RUN apt-get -y update; \
#    apt-get install -y --no-install-recommends \
#      add_package_1 \
#      add_package_2 \
#      ; \
#    apt-get clean; \
#    rm -rf /var/lib/apt/lists/*; \

# Install Centrifuge chain's executable
COPY --from=builder /workspace/target/$ARG_PROFILE/centrifuge-chain /usr/local/bin

# Shrink final image size
RUN rm -rf /usr/lib/python* ; \
	rm -rf /usr/share/man

# Expose various Substrate ports
EXPOSE 30333 9933 9944

CMD ["/usr/local/bin/centrifuge-chain"]


# -----------------------------------------------------------------------------
# Default stage (build Centrifuge chain Docker image)
# -----------------------------------------------------------------------------

FROM service

ARG ARG_CHAIN_VERSION="v2.0.0-rc6-241-1"
ARG ARG_IMAGE_BUILD_DATE="unset"
ARG ARG_PROFILE="release"