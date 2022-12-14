# Based from https://github.com/paritytech/substrate/blob/master/.maintain/Dockerfile

FROM phusion/baseimage:focal-1.0.0 as builder
LABEL maintainer="Centrifuge Team"
LABEL description="This is the build stage for the Centrifuge Chain client. Here the binary is created."

ARG RUST_TOOLCHAIN=nightly
ENV DEBIAN_FRONTEND=noninteractive
ENV RUST_TOOLCHAIN=$RUST_TOOLCHAIN

ARG PROFILE=release
ARG OPTS=""
WORKDIR /centrifuge-chain

COPY . /centrifuge-chain

RUN apt-get update && \
	apt-get dist-upgrade -y -o Dpkg::Options::="--force-confold" && \
	apt-get install -y cmake pkg-config libssl-dev git clang libclang-dev protobuf-compiler

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y && \
	export PATH="$PATH:$HOME/.cargo/bin" && \
	rustup default $RUST_TOOLCHAIN && \
	rustup target add wasm32-unknown-unknown --toolchain $RUST_TOOLCHAIN && \
	cargo build "--$PROFILE" $OPTS

# ===== SECOND STAGE ======

FROM phusion/baseimage:focal-1.0.0
LABEL maintainer="Centrifuge Team"
LABEL description="This is the 2nd stage: a very small image that contains the centrifuge-chain binary and will be used by users."
ARG PROFILE=release

RUN mv /usr/share/ca* /tmp && \
	rm -rf /usr/share/*  && \
	mv /tmp/ca-certificates /usr/share/ && \
	mkdir -p /root/.local/share/centrifuge-chain && \
    ln -s /root/.local/share/centrifuge-chain /data
    # && \
    # useradd -m -u 1000 -U -s /bin/sh -d /centrifuge-chain centrifuge-chain # commented out since users do not seem to work with PVCs we currently use: https://stackoverflow.com/questions/46873796/allowing-access-to-a-persistentvolumeclaim-to-non-root-user/46907452

COPY --from=builder /centrifuge-chain/target/$PROFILE/centrifuge-chain /usr/local/bin

# checks
RUN ldd /usr/local/bin/centrifuge-chain && \
	/usr/local/bin/centrifuge-chain --version

# Shrinking
RUN rm -rf /usr/lib/python* && \
	rm -rf /usr/bin /usr/sbin /usr/share/man

# Add chain resources to image
COPY res /resources/

# USER centrifuge-chain # see above
EXPOSE 30333 9933 9944
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/centrifuge-chain"]
