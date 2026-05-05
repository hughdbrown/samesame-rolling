#!/bin/sh

set -euo pipefail

cargo build --release
cp target/release/samesame /usr/local/bin/.
