#!/bin/bash
set -eux
TAG="$1"
REPO_ROOT=$(git rev-parse --show-toplevel)
VERSION=$(sver calc .)
LINUX_TMP=$(mktemp -d)
WINDOWS_TMP=$(mktemp -d)
MACOS_TMP=$(mktemp -d)

cd "$REPO_ROOT"
gh run download -n "sver-linux-${VERSION}" --dir "$LINUX_TMP"
cd "$LINUX_TMP"
chmod +x sver
zip "sver_${TAG}_linux_amd64.zip" *
echo "$LINUX_TMP/sver_${TAG}_linux_amd64.zip"

cd "$REPO_ROOT"
gh run download -n "sver-windows-${VERSION}" --dir "$WINDOWS_TMP"
cd "$WINDOWS_TMP"
zip "sver_${TAG}_windows_amd64.zip" *
echo "$WINDOWS_TMP/sver_${TAG}_windows_amd64.zip"

cd "$REPO_ROOT"
gh run download -n "sver-macos-${VERSION}" --dir "$MACOS_TMP"
cd "$MACOS_TMP"
chmod +x sver
zip "sver_${TAG}_macos_amd64.zip" *
echo "$MACOS_TMP/sver_${TAG}_macos_amd64.zip"

cd "$REPO_ROOT"
gh release create $TAG --generate-notes
gh release upload $TAG \
   "$LINUX_TMP/sver_${TAG}_linux_amd64.zip" \
   "$WINDOWS_TMP/sver_${TAG}_windows_amd64.zip" \
   "$MACOS_TMP/sver_${TAG}_macos_amd64.zip"
