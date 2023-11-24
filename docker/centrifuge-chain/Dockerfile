# Inspired by
# https://github.com/paritytech/polkadot-sdk/blob/master/docker/dockerfiles/polkadot/polkadot_injected_release.Dockerfile

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

	COPY . centrifuge-chain
	WORKDIR /centrifuge-chain
	ARG FEATURES=""
	RUN	FEATURES=$(echo ${FEATURES} | tr -d '"') \
		cargo build "--release" --features=${FEATURES}


# ===== SECOND STAGE ======
# IF we were to publish binaries that we can always trust we could skip the first part
# and simply download the binaries here. Some short of hash or integrity check would need to
# happen to make sure the binary is what we want.
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

	COPY --from=builder /centrifuge-chain/target/release/centrifuge-chain /usr/local/bin

	RUN apt-get autoremove -y && apt-get clean && rm -rf /var/lib/apt/lists/* ;
	
	# Because of the following, all data and config directories need to be owned by UID and GID 1000
	RUN useradd -m -u 1000 -U -s /bin/sh -d /centrifuge centrifuge
	
	RUN mkdir -p /data && \ 
		chown -R centrifuge:centrifuge /data && \
		chown -R centrifuge:centrifuge /resources && \
		chown -R centrifuge:centrifuge /usr/local/bin/centrifuge-chain && \
		chown -R centrifuge:centrifuge /centrifuge/

# Running as an non-root is a good security practice
# in some cases the container can be forced to run as root overriding the next line
# but by default we want to enforce this.
USER centrifuge
# checks
RUN ldd /usr/local/bin/centrifuge-chain && \
	/usr/local/bin/centrifuge-chain --version
ENV RUST_BACKTRACE 1
EXPOSE 30333 9933 9944
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/centrifuge-chain"]
CMD ["--help"]
