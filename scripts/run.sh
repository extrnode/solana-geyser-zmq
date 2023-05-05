#!/usr/bin/env bash

set -ex

cargo build --release
RUST_LOG=solana_geyser_plugin_scaffold solana-test-validator \
 --geyser-plugin-config config/geyser-plugin-config.json
