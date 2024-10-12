#!/usr/bin/env bash
set -euxo pipefail
cd "$(dirname "$0")"

./md-render/build.sh

export RUSTFLAGS="--remap-path-prefix=$(pwd)=~"

for name in $(ls $HOME/.cargo/registry/src); do
  registry="$HOME/.cargo/registry/src/$name"
  export RUSTFLAGS="$RUSTFLAGS --remap-path-prefix=$registry=cargo"
done

cargo build --release
