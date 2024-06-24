#!/bin/bash
echo "Please select the environment (development, demo):"
read -r ENVIRONMENT

# Check if the privateKey is empty for demo environment
if [ "$ENVIRONMENT" == "demo" ]; then
  PRIVATE_KEY=$(jq -r '.privateKey' ./config.json)
  if [ -z "$PRIVATE_KEY" ]; then
    echo "Error: privateKey is empty in ./configs/demo.json. Please retrieve it from 1Password."
    exit 1
  fi
fi

# # Install NVM and node if not present in your mac:
# brew install nvm && echo 'export NVM_DIR="$HOME/.nvm"' >> ~/.zshrc && echo '[ -s "$NVM_DIR/nvm.sh" ] \
# && \. "$NVM_DIR/nvm.sh"' >> ~/.zshrc && source ~/.zshrc && nvm install node

# Define the tag and calculate the short git hash
TAG="v0.11.0-rc3"
GIT_HASH=$(git rev-parse --short $TAG)

# Download the WASM file from Google Cloud Storage
echo "Downloading WASM file..."
if [ "$ENVIRONMENT" == "demo" ]; then
  gsutil cp gs://centrifuge-wasm-repo/development/development-"$GIT_HASH".wasm ./development.wasm
else
  gsutil cp gs://centrifuge-wasm-repo/"${ENVIRONMENT}"/"${ENVIRONMENT}"-"$GIT_HASH".wasm ./"${ENVIRONMENT}".wasm
fi

# Copy the corresponding configuration file
echo "Copying configuration file..."
cp ./configs/"${ENVIRONMENT}".json ./config.json

# Run the node script
echo "Running node index.js..."
node index.js
echo "Cleaning up..."
rm ./config.json
rm ./"${ENVIRONMENT}".wasm

