#!/bin/bash
# Used by centrifuge Linux Docker image docker/centrifuge-chain/Dockerfile
set -eux
# Define URLs and file names
URL="https://github.com/mozilla/sccache/releases/download/v0.5.4/"
TARBALL_URL="${URL}/sccache-v0.5.4-aarch64-unknown-linux-musl.tar.gz"
CHECKSUM_URL="${URL}/sccache-v0.5.4-aarch64-unknown-linux-musl.tar.gz.sha256"
TARBALL_FILENAME="sccache.tar.gz"
CHECKSUM_FILENAME="sccache.sha256"

# Define the target directory where you want to extract the binary
TARGET_DIR="/usr/local/cargo/bin"

# Download the tarball and checksum
echo "Downloading tarball..."
curl -L "$TARBALL_URL" -o "$TARBALL_FILENAME"

echo "Downloading checksum..."
curl -L "$CHECKSUM_URL" -o "$CHECKSUM_FILENAME"

# Verify the checksum
echo "Verifying checksum..."
EXPECTED_SHA256=$(cat "$CHECKSUM_FILENAME" | awk '{print $1}')
ACTUAL_SHA256=$(sha256sum "$TARBALL_FILENAME" | awk '{print $1}')

if [ "$ACTUAL_SHA256" != "$EXPECTED_SHA256" ]; then
  echo "Checksum verification failed. Aborting."
  rm "$TARBALL_FILENAME" "$CHECKSUM_FILENAME"
  exit 1
fi

# Extract the tarball
echo "Extracting tarball..."
mkdir sccache
tar -vxzf sccache.tar.gz -C ./sccache/ --strip-components 1

# Copy the sccache binary to the target directory
echo "Copying sccache binary to $TARGET_DIR"
cp "sccache/sccache" "$TARGET_DIR/"

# Clean up downloaded files and extracted folder
rm "$TARBALL_FILENAME" "$CHECKSUM_FILENAME"
rm -rf "sccache"

echo "Installation completed successfully."
