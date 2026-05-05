#!/bin/sh

set -euo pipefail

git tag -a v1.2.1 -m "Release 1.2.1"
git push origin v1.2.1

cargo publish
