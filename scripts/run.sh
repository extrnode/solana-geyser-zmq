#!/usr/bin/env bash

set -ex

cargo build
 RUST_LOG=solana_geyser_plugin_scaffold::geyser_plugin_hook solana-test-validator \
 --geyser-plugin-config config/geyser-plugin-config.json
