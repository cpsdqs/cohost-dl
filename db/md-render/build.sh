#!/bin/bash
set -euxo pipefail
cd "$(dirname "$0")"

mkdir -p src/cohost
cp -r ../../out/~src/a2ecdc59/* src/cohost/

cd src/cohost/
patch -p1 -u -i ../awawawa.patch

cd ../..

npm ci
npm run build
