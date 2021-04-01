
###############################################################################
# Centrifuge                                                                  #
# Cash on Steroids                                                            #
#                                                                             #
# tools/docker/sandbox/Dockerfile                                             #
#                                                                             #
# Handcrafted since 2020 by Centrifugians                                     #
# All rights reserved                                                         #
#                                                                             #
#                                                                             #
# Description: Developer sandbox container's entrypoint script.               #
###############################################################################

FROM debian:buster-slim

ARG ARG_RUST_VERSION="1.51.0"
ARG ARG_RUSTUP_VERSION="1.23.1"
ARG ARG_RUST_TOOLCHAIN="nightly-2020-08-16"
ARG ARG_IMAGE_BUILD_DATE=""
ARG ARG_IMAGE_VERSION=""

LABEL io.centrifuge.chain.sandbox.maintainer="centrifugians@centrifuge.io" \
      io.centrifuge.chain.sandbox.image.version="$ARG_IMAGE_VERSION" \
      io.centrifuge.chain.sandbox.image.description="Developer sandbox for building and testing Centrifuge chain" \
      io.centrifuge.chain.sandbox.image.vendorName="Centrifuge" \
      io.centrifuge.chain.sandbox.image.buildDate="$ARG_IMAGE_BUILD_DATE" 

# Define environment variables (see also 'scripts/docker-entrypoint.sh' file)
ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo
ENV CARGO_REGISTRY_PATH=$CARGO_HOME/registry
ENV PATH=$CARGO_HOME/bin:$PATH
ENV RUST_VERSION=$ARG_RUST_VERSION
ENV RUSTUP_VERSION=$ARG_RUSTUP_VERSION
ENV RUST_TOOLCHAIN=$ARG_RUST_TOOLCHAIN
ENV WORKSPACE_PATH=/workspace
ENV PROJECT_CARGO_REGISTRY_PATH=$WORKSPACE_PATH/.cargo/registry

# Exposed TCP/IP ports
EXPOSE 30333 9933 9944

# Install packages
RUN set -eux; \
    apt-get -y update; \
    # Install default packages (i.e. clan, ssl, ...)
    apt-get install -y --no-install-recommends \
		  libssl-dev clang lld libclang-dev make cmake \
		  git pkg-config curl time rhash ca-certificates jq \
      ca-certificates \
      toilet \
      gcc \
      libc6-dev \
      wget \
      ; \
    # Set a link to Clang
	  update-alternatives --install /usr/bin/cc cc /usr/bin/clang 100; \
    # Detect machine architecture
    dpkgArch="$(dpkg --print-architecture)"; \
    case "${dpkgArch##*-}" in \
        amd64) rustArch='x86_64-unknown-linux-gnu'; rustupSha256='ed7773edaf1d289656bdec2aacad12413b38ad0193fff54b2231f5140a4b07c5' ;; \
        armhf) rustArch='armv7-unknown-linux-gnueabihf'; rustupSha256='7a7b9d246ad63358705d8d4a7d5c2ef1adfec24525d1d5c44a7739e1b867e84d' ;; \
        arm64) rustArch='aarch64-unknown-linux-gnu'; rustupSha256='f80a0a792b3ab905ab4919474daf4d3f60e574fc6987e69bfba2fd877241a8de' ;; \
        i386) rustArch='i686-unknown-linux-gnu'; rustupSha256='4473c18286aa1831683a772706d9a5c98b87a61cc014d38063e00a63a480afef' ;; \
        *) echo >&2 "unsupported architecture: ${dpkgArch}"; exit 1 ;; \
    esac; \
    # Install Rustup
    wget "https://static.rust-lang.org/rustup/archive/1.23.1/$rustArch/rustup-init"; \
    echo "$rustupSha256 *rustup-init" | sha256sum -c -; \
    chmod +x rustup-init; \
    ./rustup-init -y --no-modify-path --profile minimal --default-toolchain $ARG_RUST_VERSION --default-host $rustArch; \
    rm rustup-init; \
    chmod -R a+w $RUSTUP_HOME $CARGO_HOME; \
    \
    # Install nightly and stable toolchains, as well as WASM
    rustup update $ARG_RUST_TOOLCHAIN; \
    rustup update stable; \
    rustup component add rust-src --toolchain $ARG_RUST_TOOLCHAIN; \
    rustup target add wasm32-unknown-unknown --toolchain $ARG_RUST_TOOLCHAIN; \
    rustup default $ARG_RUST_TOOLCHAIN; \
    \
    # Clean up Cargo index and cache (generally more than 250MB)
	  rm -rf $CARGO_REGISTRY_PATH $CARGO_HOME/git /root/.cache/sccache; \
    \
    # Remove unecessary packages and clean up package manager's cache
    apt-get remove -y --auto-remove \
      wget; \
    apt-get clean; \
    rm -rf /var/lib/apt/lists/*;

# Install container's entry point script
COPY ./scripts/docker-entrypoint.sh /
RUN chmod 755 /docker-entrypoint.sh

# Set working directory (on which the project to be compiled should be mounted, when running a container)
WORKDIR $WORKSPACE_PATH

# Volume on which Centrifuge chain's code is mounted
VOLUME $WORKSPACE_PATH

# Bootstrap container using an entrypoint script
ENTRYPOINT [ "/docker-entrypoint.sh" ]
