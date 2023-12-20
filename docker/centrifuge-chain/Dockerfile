# Inspired by
# https://github.com/paritytech/polkadot-sdk/blob/master/docker/dockerfiles/polkadot/polkadot_injected_release.Dockerfile

FROM --platform=linux/amd64 docker.io/paritytech/ci-linux:production as builder
  # Defaults
	ENV RUST_BACKTRACE 1

  # RustUp
	COPY ./scripts scripts
	COPY rust-toolchain.toml ./
	# RUN rustup update && rustup default
	ENV PATH=~/.cargo/bin:$PATH
	RUN	scripts/install_toolchain.sh

  # BUILD
	ARG RUSTC_WRAPPER=''
	ARG SCCACHE_GHA_ENABLED="false"

	COPY . /centrifuge-chain
	WORKDIR /centrifuge-chain
	ARG FEATURES=""
	RUN	FEATURES=$(echo ${FEATURES} | tr -d '"') \
		cargo build -p centrifuge-chain --locked --release --features=${FEATURES}

# ===== SECOND STAGE ======
# IF we were to publish binaries that we can always trust we could skip the first part
# and simply download the binaries here. Some short of hash or integrity check would need to
# happen to make sure the binary is what we want.
FROM --platform=linux/amd64 docker.io/library/ubuntu:jammy
	ARG BUILD_DATE
	ARG VERSION
	# Standard OCI labels
	# https://github.com/opencontainers/image-spec/blob/main/annotations.md
	LABEL io.centrifuge.image.vendor="Centrifuge" \
		org.opencontainers.image.created= \
		org.opencontainers.image.authors="protocol@k-f.co" \
		org.opencontainers.image.title="centrifugeio/centrifuge-chain" \
		org.opencontainers.image.description="Centrifuge, the layer 1 of RWA. This is the official Centrifuge (para)chain image" \
		org.opencontainers.image.source="https://github.com/centrifuge/centrifuge-chain/" \
		org.opencontainers.image.created="${BUILD_DATE}" \		
		org.opencontainers.image.documentation="https://github.com/centrifuge/centrifuge-chain/blob/main/README.md" \
		org.opencontainers.image.version="$VERSION" \
		org.opencontainers.image.licenses="GPL-3" \
		org.opencontainers.image.base.name="ubuntu/jammy" \
		org.opencontainers.image.base.digest="ubuntu@sha256:6042500cf4b44023ea1894effe7890666b0c5c7871ed83a97c36c76ae560bb9b" 
		# org.opencontainers.image.url= \		



	RUN apt-get update && apt-get install -y curl jq
	RUN apt-get autoremove -y && apt-get clean && rm -rf /var/lib/apt/lists/*

	# Because of the following, all data and config directories need to be owned by UID and GID 1000
	RUN useradd -m -u 1000 -U -s /bin/sh -d /centrifuge centrifuge
	
	COPY node/res /resources/
	COPY --from=builder /centrifuge-chain/target/release/centrifuge-chain /usr/local/bin/

	COPY docker/scripts /centrifuge/scripts
	RUN chmod +x /centrifuge/scripts/* && \
		ln -s /centrifuge/scripts/* /usr/local/bin/

	RUN mkdir -p /data && \ 
		chown -R centrifuge:centrifuge /data && \
		chown -R centrifuge:centrifuge /resources && \
		chown -R centrifuge:centrifuge /centrifuge/ 

# Running as an non-root is a good security practice
# in some cases the container can be forced to run as root overriding the next line
# but by default we want to enforce this.
USER centrifuge
WORKDIR /centrifuge

# checks
RUN ldd /usr/local/bin/centrifuge-chain && \
	/usr/local/bin/centrifuge-chain --version
RUN set -e && command -v check-node-ready && command -v entrypoint.sh

EXPOSE 30333 9933 9944
VOLUME ["/data"]
ENTRYPOINT ["entrypoint.sh"]
CMD ["--help"]
