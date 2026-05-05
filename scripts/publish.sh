#!/bin/sh

set -euo pipefail

git tag -a v1.3.0 -m "Release 1.3.0"
git push origin v1.3.0

cargo publish
