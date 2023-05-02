#!/usr/bin/env bash

# Source:
#https://github.com/Blockdaemon/solana-accountsdb-plugin-kafka/blob/main/ci/solana-version.sh

# Prints the Solana version.

set -e

cd "$(dirname "$0")/.."

cargo read-manifest | jq -r '.dependencies[] | select(.name == "solana-geyser-plugin-interface") | .req'
