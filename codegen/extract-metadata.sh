#!/bin/bash
VERSION=1.3.0
TARBALL=https://github.com/svrooij/sonos-api-docs/archive/refs/tags/v${VERSION}.tar.gz
curl -L $TARBALL | tar xz sonos-api-docs-${VERSION}/generator/sonos-docs/data sonos-api-docs-${VERSION}/docs/documentation.json
mkdir -p data/devices
mv sonos-api-docs-${VERSION}/generator/sonos-docs/data/*.json data/devices
mv sonos-api-docs-${VERSION}/docs/documentation.json data/
rmdir sonos-api-docs-${VERSION}/generator/sonos-docs/data \
  sonos-api-docs-${VERSION}/generator/sonos-docs/ \
  sonos-api-docs-${VERSION}/generator/ \
  sonos-api-docs-${VERSION}/docs \
  sonos-api-docs-${VERSION}

