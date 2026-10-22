#!/usr/bin/env bash
set -euo pipefail

: "${VERSION:?VERSION is required}"

echo "Building LinuPrint $VERSION in Arch Linux"

if ! pacman -Syu --noconfirm --needed --noprogressbar \
  base-devel rust gtk4 pkgconf git cups avahi sane polkit xdg-utils hicolor-icon-theme \
  >/tmp/pacman.log 2>&1; then
  tail -n 80 /tmp/pacman.log
  exit 1
fi

rm -rf /tmp/build
mkdir -p /tmp/build
cp -a /src/. /tmp/build/

cd /tmp/build/packaging
pkgver="$(sed -n 's/^pkgver=//p' PKGBUILD | head -n 1)"
if [ "$pkgver" != "$VERSION" ]; then
  echo "Tag version $VERSION does not match pkgver $pkgver" >&2
  exit 1
fi

useradd -m builder
chown -R builder /tmp/build /out

if ! su builder -c 'cd /tmp/build/packaging && makepkg --printsrcinfo' >/tmp/srcinfo; then
  echo 'Failed to generate .SRCINFO' >&2
  exit 1
fi
if ! diff -u .SRCINFO /tmp/srcinfo; then
  echo '.SRCINFO is out of date; run makepkg --printsrcinfo > .SRCINFO' >&2
  exit 1
fi

if ! su builder -c 'cd /tmp/build/packaging && makepkg -f --noconfirm'; then
  echo 'makepkg failed' >&2
  exit 1
fi

shopt -s nullglob
packages=(/tmp/build/packaging/*.pkg.tar.zst)
found=0
for package in "${packages[@]}"; do
  case "$package" in
    *-debug-*) continue ;;
  esac
  cp "$package" /out/
  found=1
done

if [ "$found" -ne 1 ]; then
  echo 'No package artifact was produced' >&2
  exit 1
fi
