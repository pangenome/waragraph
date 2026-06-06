#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/../.."
cd ..

cargo test -p waragraph viewer_1d::view::tests
