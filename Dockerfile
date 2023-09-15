# Based on
# https://github.com/paritytech/polkadot-sdk/blob/master/docker/dockerfiles/polkadot/polkadot_injected_release.Dockerfile
FROM docker.io/library/ubuntu:22.04 as builder

	# Defaults
	ENV RUST_BACKTRACE 1
	ENV DEBIAN_FRONTEND=noninteractive
	ENV RUST_TOOLCHAIN=$RUST_TOOLCHAIN
	ARG FEATURES=""
	ARG RUST_TOOLCHAIN="1.66"
	
	RUN apt-get update && \
		# apt-get dist-upgrade -y -o Dpkg::Options::="--force-confold" && \
		apt-get install -y \
			cmake \
			pkg-config \
			libssl-dev \
			git \
			clang \
			libclang-dev \
			protobuf-compiler \
			curl
	
	RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
	ENV PATH="${PATH}:/root/.cargo/bin"
	
	# BUILD
	COPY . centrifuge-chain
	WORKDIR /centrifuge-chain
	RUN echo $(ls -l /centrifuge-chain/)

	RUN	rustup default $RUST_TOOLCHAIN && \
		rustup target add wasm32-unknown-unknown --toolchain $RUST_TOOLCHAIN && \
		cargo build "--release" --features=${FEATURES}

# ===== SECOND STAGE ======

FROM docker.io/library/ubuntu:22.04

	LABEL io.centrifuge.image.authors="guillermo@k-f.co" \
		io.centrifuge.image.vendor="Centrifuge" \
		io.centrifuge.image.title="centrifugeio/centrifuge-chain" \
		io.centrifuge.image.description="Centrifuge, the layer 1 of RWA. This is the official Centrifuge image with an injected binary." \
		io.centrifuge.image.source="https://github.com/centrifuge/centrifuge-chain/blob/main/Dockerfile" \
		# io.centrifuge.image.revision="${VCS_REF}" \
		io.centrifuge.image.created="${BUILD_DATE}"

	COPY --from=builder /centrifuge-chain/target/release/centrifuge-chain /usr/local/bin

	RUN useradd -m -u 1000 -U -s /bin/sh -d /centrifuge centrifuge && \
			mkdir -p /data /centrifuge/.local/share && \
			chown -R centrifuge:centrifuge /data && \
			ln -s /data /centrifuge/.local/share/centrifuge

	# checks
	RUN ldd /usr/local/bin/centrifuge-chain && \
		/usr/local/bin/centrifuge-chain --version

	# Save sh and bash
	RUN cp /usr/bin/sh /usr/bin/bash /usr/local/bin/ /root/

	# Unclutter
	RUN mv /usr/share/ca* /tmp && \
		rm -rf /usr/share/*  && \
		mv /tmp/ca-certificates /usr/share/ && \
		mkdir -p /root/.local/share/centrifuge-chain && \
		ln -s /root/.local/share/centrifuge-chain /data \
	# minimize the attack surface
		rm -rf /usr/bin /usr/sbin && \
		rm -rf /usr/lib/python* && \
	# check if executable works in this container
		ldd /usr/local/bin/centrifuge-chain && \
		/usr/local/bin/centrifuge-chain --version

	# Add chain resources to image
	COPY res /resources/

USER centrifuge
EXPOSE 30333 9933 9944
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/centrifuge-chain"]
