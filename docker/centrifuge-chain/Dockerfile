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

	RUN apt-get update && apt-get install -y curl jq
	RUN apt-get autoremove -y && apt-get clean && rm -rf /var/lib/apt/lists/*

	# Because of the following, all data and config directories need to be owned by UID and GID 1000
	RUN useradd -m -u 1000 -U -s /bin/sh -d /centrifuge centrifuge
	
	COPY node/res /resources/
	COPY --from=builder /centrifuge-chain/target/release/centrifuge-chain /usr/local/bin/

	COPY docker/scripts /centrifuge/scripts
	# The following will make everything under /centrifuge/scripts reachable from PATH
	# by creating a symbolic link into /usr/local/bin
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

EXPOSE 30333 9933 9944
VOLUME ["/data"]
ENTRYPOINT ["entrypoint.sh"]
CMD ["--help"]
