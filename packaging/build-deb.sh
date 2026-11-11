#!/usr/bin/env bash
# Build target/ember_<version>_<arch>.deb with dpkg-deb (no extra tools).
# Usage: packaging/build-deb.sh [--no-build]
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
ARCH=$(dpkg --print-architecture)
[[ "${1:-}" == "--no-build" ]] || cargo build --release --locked -p ember -p ember-applet -p ember-widget -p emberd -p ember-helper

STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT
R="$STAGE/root"
install -Dm755 target/release/ember          "$R/usr/bin/ember"
install -Dm755 target/release/ember-applet   "$R/usr/bin/ember-applet"
install -Dm755 target/release/ember-widget   "$R/usr/bin/ember-widget"
install -Dm755 target/release/emberd         "$R/usr/bin/emberd"
install -Dm755 install.sh                     "$R/usr/bin/ember-install"
install -Dm755 target/release/ember-helper   "$R/usr/libexec/ember/ember-helper"
for f in res/applications/*.desktop; do install -Dm644 "$f" "$R/usr/share/applications/$(basename "$f")"; done
for n in 16 24 32 48 64 128 256 512; do
    install -Dm644 "res/icons/ember-$n.png" "$R/usr/share/icons/hicolor/${n}x${n}/apps/io.github.n1th1n_19.Ember.png"
done
install -Dm644 res/polkit/io.github.n1th1n_19.Ember.helper.policy "$R/usr/share/polkit-1/actions/io.github.n1th1n_19.Ember.helper.policy"
install -Dm644 res/systemd/ember.service      "$R/usr/lib/systemd/user/ember.service"
install -Dm644 res/metainfo/io.github.n1th1n_19.Ember.metainfo.xml "$R/usr/share/metainfo/io.github.n1th1n_19.Ember.metainfo.xml"
install -Dm644 packaging/debian/copyright      "$R/usr/share/doc/ember/copyright"
gzip -9n -c CHANGELOG.md > "$STAGE/changelog.gz" && install -Dm644 "$STAGE/changelog.gz" "$R/usr/share/doc/ember/changelog.gz"
for f in res/fonts/*.txt; do install -Dm644 "$f" "$R/usr/share/doc/ember/fonts/$(basename "$f")"; done

mkdir -p "$R/DEBIAN"
install -m755 packaging/debian/postinst packaging/debian/postrm "$R/DEBIAN/"
SIZE=$(du -sk --exclude=DEBIAN "$R" | cut -f1)
cat > "$R/DEBIAN/control" <<CONTROL
Package: ember
Version: $VERSION
Architecture: $ARCH
Maintainer: n1th1n-19 <n1th1n-19@users.noreply.github.com>
Installed-Size: $SIZE
Depends: libc6 (>= 2.35), libgcc-s1, libxkbcommon0, libwayland-client0, libegl1, libvulkan1, pkexec | policykit-1
Recommends: pulseaudio-utils, network-manager, bluez, rfkill, libglib2.0-bin, system76-power
Section: utils
Priority: optional
Homepage: https://github.com/n1th1n-19/ember
Description: comic-style system monitor and control center for COSMIC
 Ember shows CPU, memory, disks, network, temperatures, battery, GPU and
 fans as hand-drawn comic panels with live charts. It controls processes,
 services, power profiles, graphics mode, CPU scaling and installed apps
 from every source. Includes COSMIC panel applets, a desktop widget and a
 history and alerts service.
CONTROL

OUT="target/ember_${VERSION}_${ARCH}.deb"
dpkg-deb --root-owner-group --build "$R" "$OUT" >/dev/null
echo "$OUT"
