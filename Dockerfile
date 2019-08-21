# https://github.com/centrifuge/substrate-builder
FROM centrifugeio/substrate-builder:latest as builder
LABEL maintainer="philip@centrifuge.io"
LABEL description="This is the build stage for the Centrifuge Chain client. Here the binary is created."

ARG PROFILE=release
WORKDIR /centrifuge-chain

COPY . /centrifuge-chain

RUN export PATH=$PATH:$HOME/.cargo/bin && \
	cargo build "--$PROFILE"

# ===== SECOND STAGE ======

FROM phusion/baseimage:0.10.0
LABEL maintainer="philip@centrifuge.io"
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
