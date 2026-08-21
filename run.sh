#!/usr/bin/env bash
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$DIR"

cmd="${1:-dev}"

case "$cmd" in
  dev)
    echo "[run] dev — compiling HTML and launching with hot-reload..."
    cargo run -p app
    ;;
  build)
    echo "[run] build — optimized release build..."
    cargo build --release -p app
    echo "[run] built target/release/app"
    ;;
  preview)
    if [ ! -f "target/release/app" ]; then
      echo "[run] no release build found. Run: ./run.sh build"
      exit 1
    fi
    echo "[run] preview — launching release build..."
    ./target/release/app
    ;;
  test)
    cargo test -p html2gpui
    ;;
  *)
    echo "Usage: ./run.sh [dev|build|preview|test]"
    exit 1
    ;;
esac
