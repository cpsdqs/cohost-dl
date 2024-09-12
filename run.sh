#!/usr/bin/env bash
cd "$(dirname "$0")"
deno run --allow-env --alow-ffi --allow-net --allow-read=out --allow-write=out main.ts
