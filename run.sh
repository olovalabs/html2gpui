#!/usr/bin/env bash
set -e
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$DIR"
cmd="${1:-dev}"
case "$cmd" in
  dev) cargo run -p app ;;
  build) cargo build --release -p app ;;
  *) echo "Usage: ./run.sh [dev|build]"; exit 1 ;;
esac
