FROM pstehlik/substrate-builder:latest as builder
LABEL maintainer "philip@centrifuge.io"
LABEL description="This is the build stage for the Centrifuge Chain client. Here the binary is created."

ARG PROFILE=release
WORKDIR /centrifuge-chain

COPY . /centrifuge-chain

# This needs to be repeated(can't use init.sh either) here because of
# rustup bug inside docker builds - https://github.com/rust-lang/rustup.rs/issues/1239
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y && \
	export PATH="$PATH:$HOME/.cargo/bin" && \
	rustup toolchain install nightly && \
	rustup target add wasm32-unknown-unknown --toolchain nightly && \
	cargo install --git https://github.com/alexcrichton/wasm-gc && \
	rustup default nightly && \
	rustup default stable

RUN export PATH=$PATH:$HOME/.cargo/bin && \
    bash ./scripts/build.sh

RUN export PATH=$PATH:$HOME/.cargo/bin && \
	cargo build "--$PROFILE"

# ===== SECOND STAGE ======

FROM phusion/baseimage:0.10.0
LABEL maintainer "philip@centrifuge.io"
LABEL description="This is the 2nd stage: a very small image that contains the centrifuge-chain binary and will be used by users."
ARG PROFILE=release
COPY --from=builder /centrifuge-chain/target/$PROFILE/centrifuge-chain /usr/local/bin

RUN mv /usr/share/ca* /tmp && \
	rm -rf /usr/share/*  && \
	mv /tmp/ca-certificates /usr/share/ && \
	rm -rf /usr/lib/python* && \
	mkdir -p /root/.local/share/centrifuge-chain && \
	ln -s /root/.local/share/centrifuge-chain /data

RUN	rm -rf /usr/bin /usr/sbin

EXPOSE 30333 9933 9944
VOLUME ["/data"]

CMD ["/usr/local/bin/centrifuge-chain"]