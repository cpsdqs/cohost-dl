#!/usr/bin/env bash
cd "$(dirname "$0")"
deno run --allow-env --allow-ffi --allow-net --allow-read --allow-write=out main.ts
