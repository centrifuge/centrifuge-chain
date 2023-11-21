# Based on
# https://github.com/paritytech/polkadot-sdk/blob/master/docker/dockerfiles/polkadot/polkadot_injected_release.Dockerfile

# ToDo: create a CI/builder image with preloaded tools
# FROM --platform=linux/amd64 docker.io/library/ubuntu:22.04 as builder
FROM --platform=linux/amd64 docker.io/library/rust:bookworm as builder
  # Defaults
	ENV RUST_BACKTRACE 1
	ENV DEBIAN_FRONTEND=noninteractive

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

	COPY ./scripts scripts
	COPY rust-toolchain.toml ./
	# RUN rustup update && rustup default
	ENV PATH=~/.cargo/bin:$PATH
	RUN	scripts/install_toolchain.sh 

  # BUILD
	ARG RUSTC_WRAPPER=''
	ARG SCCACHE_GHA_ENABLED="false"

	RUN	rustup target add wasm32-unknown-unknown

	COPY . centrifuge-chain
	WORKDIR /centrifuge-chain
	ARG FEATURES=""
	RUN	FEATURES=$(echo ${FEATURES} | tr -d '"') \
		cargo build "--release" --features=${FEATURES}


# ===== SECOND STAGE ======
# ToDo: create a secure image as a base for the binary
FROM --platform=linux/amd64 docker.io/library/ubuntu:jammy

	LABEL io.centrifuge.image.authors="guillermo@k-f.co" \
		io.centrifuge.image.vendor="Centrifuge" \
		io.centrifuge.image.title="centrifugeio/centrifuge-chain" \
		io.centrifuge.image.description="Centrifuge, the layer 1 of RWA. This is the official Centrifuge image with an injected binary." \
		io.centrifuge.image.source="https://github.com/centrifuge/centrifuge-chain/blob/main/docker/centrifuge-chain/Dockerfile" \
		# io.centrifuge.image.revision="${VCS_REF}" \
		io.centrifuge.image.created="${BUILD_DATE}"

	# Add chain resources to image
	COPY res /resources/

	RUN useradd -m -u 1000 -U -s /bin/sh -d /centrifuge centrifuge && \
			mkdir -p /data /centrifuge/.local/share && \
			chown -R centrifuge:centrifuge /data && \
			ln -s /data /centrifuge/.local/share/centrifuge

	COPY --from=builder /centrifuge-chain/target/release/centrifuge-chain /usr/local/bin
	# checks
	RUN ldd /usr/local/bin/centrifuge-chain && \
		/usr/local/bin/centrifuge-chain --version

	RUN apt-get autoremove -y && apt-get clean && rm -rf /var/lib/apt/lists/* ;
	RUN	mkdir -p /root/.local/share/centrifuge-chain && \
		ln -s /root/.local/share/centrifuge-chain /data


ENV RUST_BACKTRACE 1
USER centrifuge
EXPOSE 30333 9933 9944
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/centrifuge-chain"]
