#!/usr/bin/env bash
set -euo pipefail

: "${RELEASE_TAG:?RELEASE_TAG is required}"
version="${RELEASE_TAG#v}"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
  echo "Invalid release tag: $RELEASE_TAG" >&2
  exit 1
fi

rm -rf dist
mkdir -p dist

docker run --rm \
  --env "VERSION=$version" \
  --volume "$PWD:/src:ro" \
  --volume "$PWD/dist:/out" \
  archlinux:latest \
  bash /src/.github/scripts/build-in-arch.sh

ls -lh dist
