#!/bin/bash
CARGO_TARGET_DIR=target cargo build --release --target wasm32-unknown-unknown --features prod
hc dna pack workdir