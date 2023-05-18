![Build on Ubuntu 20 badge](https://github.com/extrnode/solana-geyser-zmq/actions/workflows/build_on_ubuntu_20.04.yml/badge.svg)
![Build on Ubuntu 22 badge](https://github.com/extrnode/solana-geyser-zmq/actions/workflows/build_on_ubuntu_22.04.yml/badge.svg)
![Release badge](https://github.com/extrnode/solana-geyser-zmq/actions/workflows/release.yml/badge.svg)

# Solana Geyser ZMQ

### What's a Solana Geyser Plugin?
A Solana Validator can _"leak"_ accounts and transactions data outside the validator.
This flow of data is achieved through the [The Geyser Plugin Interface.](https://docs.rs/solana-geyser-plugin-interface/latest/solana_geyser_plugin_interface/geyser_plugin_interface/trait.GeyserPlugin.html)

An external library can _plug_ into that interface by implementing the necessary functions and thus listen for accounts and transactions streams.

That dynamic library is provided to the validator at runtime. The validator can then open that library and call the implemented _callbacks_ or _hooks_ with accounts and transactions data.

The library can then feed on these data and take further actions, such as logging, inserting the data into a DB or a consumer/producer system, etc.

### Building
If a specific rust version not used for building, segmentation faults occur during validator start. Current mainnet ver is 1.13.6 so it's recommended to use rust 1.60.0 [issue #30140](https://github.com/solana-labs/solana/issues/30140#issuecomment-1418796314).
```
    docker run --rm -v $(PWD):/app -w /app rust:1.60.0 cargo b --release
```

### Testing
The dynamic library path is provided to the validator using the `--geyser-plugin-config` parameter.

For example when using the test validator:
```bash
solana-test-validator --geyser-plugin-config config/geyser-plugin-config.json
# or use ./scripts/run.sh
```

Make sure the path to your geyser plugin dynamic library has a proper path relative to a config file and a proper extenstion -  _.so_ or (_.dylib_ on mac)

For example:
```json
{
    "libpath": "../target/release/libsolana_geyser_plugin_scaffold.so",
}
```
