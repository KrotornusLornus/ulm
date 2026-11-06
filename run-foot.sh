#!/bin/sh
cd "$(dirname "$0")" || exit 1
exec footclient -F \
  -o font='DejaVu Sans Mono:pixelsize=26.6' \
  -o line-height=32px \
  ./target/release/ulm --check "$@"
