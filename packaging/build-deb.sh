#!/usr/bin/env bash
# Build Ember's .deb packages into target/debs/ with dpkg-deb (no extra tools).
#
#   ember-common          icons, licenses, the ember-install script (everything depends on it)
#   ember                 the app + root helper + polkit policy
#   ember-service         emberd (history + alerts) + systemd user unit
#   ember-widget          desktop widget
#   ember-applet          panel applet engine (pulled in by the applet packages)
#   ember-applet-<name>   one per panel applet (readings, clock, sound, ...)
#   ember-full            installs all of the above
#
# Usage: packaging/build-deb.sh [--no-build]
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
ARCH=$(dpkg --print-architecture)
OUT=target/debs
MAINT="n1th1n-19 <n1th1n-19@users.noreply.github.com>"
HOME_URL="https://github.com/n1th1n-19/ember"
LIBS="libc6 (>= 2.35), libgcc-s1, libxkbcommon0, libwayland-client0, libegl1, libvulkan1"
COMMON="ember-common (= $VERSION)"
# release date of this version from CHANGELOG.md ("## 0.1.0 — 2026-11-11"), RFC 2822
RELEASE_DATE=$(date -R -u -d "$(sed -n "s/^## $VERSION .* \([0-9-]\{10\}\)$/\1/p" CHANGELOG.md | head -1) 12:00")
APPLETS=(readings clock sound bluetooth network power dnd tiling session media awake)

[[ "${1:-}" == "--no-build" ]] || cargo build --release --locked -p ember -p ember-applet -p ember-widget -p emberd -p ember-helper
rm -rf "$OUT" && mkdir -p "$OUT"
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

# pkg <name> <arch> <depends> <recommends> <summary> <long description>; files already staged in $STAGE/<name>
pkg() {
    local name=$1 arch=$2 depends=$3 recommends=$4 summary=$5 long=$6 R="$STAGE/$1"
    install -Dm644 packaging/debian/copyright "$R/usr/share/doc/$name/copyright"
    printf '%s (%s) stable; urgency=medium\n\n  * Release %s. Full notes: %s/blob/main/CHANGELOG.md\n\n -- %s  %s\n' \
        "$name" "$VERSION" "$VERSION" "$HOME_URL" "$MAINT" "$RELEASE_DATE" | gzip -9n > "$STAGE/$name.changelog.gz"
    install -Dm644 "$STAGE/$name.changelog.gz" "$R/usr/share/doc/$name/changelog.gz"
    mkdir -p "$R/DEBIAN"
    for s in postinst prerm postrm; do [[ -f "packaging/debian/$name.$s" ]] && install -m755 "packaging/debian/$name.$s" "$R/DEBIAN/$s"; done
    {
        echo "Package: $name"
        echo "Version: $VERSION"
        echo "Architecture: $arch"
        echo "Maintainer: $MAINT"
        echo "Installed-Size: $(du -sk --exclude=DEBIAN "$R" | cut -f1)"
        [[ -n "$depends" ]] && echo "Depends: $depends"
        [[ -n "$recommends" ]] && echo "Recommends: $recommends"
        echo "Section: utils"
        echo "Priority: optional"
        echo "Homepage: $HOME_URL"
        echo "Description: $summary"
        printf '%s\n' "$long" | fold -s -w 72 | sed 's/^/ /;s/ *$//'
    } > "$R/DEBIAN/control"
    dpkg-deb --root-owner-group --build "$R" "$OUT/${name}_${VERSION}_${arch}.deb" >/dev/null
    echo "$OUT/${name}_${VERSION}_${arch}.deb"
}

# ember-common
R="$STAGE/ember-common"
install -Dm755 install.sh "$R/usr/bin/ember-install"
for n in 16 24 32 48 64 128 256 512; do
    install -Dm644 "res/icons/ember-$n.png" "$R/usr/share/icons/hicolor/${n}x${n}/apps/io.github.n1th1n_19.Ember.png"
done
gzip -9n -c CHANGELOG.md > "$STAGE/NEWS.md.gz"
install -Dm644 "$STAGE/NEWS.md.gz" "$R/usr/share/doc/ember-common/NEWS.md.gz"
for f in res/fonts/*.txt; do install -Dm644 "$f" "$R/usr/share/doc/ember-common/fonts/$(basename "$f")"; done
pkg ember-common all "" "" "Ember shared files (icon, licenses, installer)" \
    "Icons, font licenses and the ember-install script (install, update or remove Ember components) shared by all Ember packages."

# ember (app)
R="$STAGE/ember"
install -Dm755 target/release/ember "$R/usr/bin/ember"
install -Dm755 target/release/ember-helper "$R/usr/libexec/ember/ember-helper"
install -Dm644 res/applications/io.github.n1th1n_19.Ember.desktop "$R/usr/share/applications/io.github.n1th1n_19.Ember.desktop"
install -Dm644 res/polkit/io.github.n1th1n_19.Ember.helper.policy "$R/usr/share/polkit-1/actions/io.github.n1th1n_19.Ember.helper.policy"
install -Dm644 res/systemd/ember-firmware.service "$R/usr/lib/systemd/system/ember-firmware.service"
install -Dm644 res/metainfo/io.github.n1th1n_19.Ember.metainfo.xml "$R/usr/share/metainfo/io.github.n1th1n_19.Ember.metainfo.xml"
pkg ember "$ARCH" "$COMMON, $LIBS, pkexec | policykit-1" "ember-service, pulseaudio-utils, network-manager, bluez, rfkill, libglib2.0-bin, system76-power" \
    "comic-style system monitor and control center for COSMIC" \
    "Ember shows CPU, memory, disks, network, temperatures, battery, GPU and fans as hand-drawn comic panels with live charts. It controls processes, services, power profiles, graphics mode, CPU scaling and installed apps from every source, and has a Quick panel for sound, Bluetooth, network and more."

# ember-service
R="$STAGE/ember-service"
install -Dm755 target/release/emberd "$R/usr/bin/emberd"
install -Dm644 res/systemd/ember.service "$R/usr/lib/systemd/user/ember.service"
pkg ember-service "$ARCH" "$COMMON, libc6 (>= 2.35), libgcc-s1" "" "Ember history and alerts service" \
    "emberd records one hour of system history for Ember's charts and sends desktop notifications when CPU, memory or CPU temperature stay high or the battery runs low. Runs as a systemd user service."

# ember-widget
R="$STAGE/ember-widget"
install -Dm755 target/release/ember-widget "$R/usr/bin/ember-widget"
install -Dm644 res/applications/io.github.n1th1n_19.Ember.Widget.desktop "$R/usr/share/applications/io.github.n1th1n_19.Ember.Widget.desktop"
pkg ember-widget "$ARCH" "$COMMON, $LIBS" "" "Ember desktop widget for COSMIC" \
    "A live readings card on the COSMIC desktop, under your windows. Start it at login with: ember-widget --autostart on"

# ember-applet (engine)
R="$STAGE/ember-applet"
install -Dm755 target/release/ember-applet "$R/usr/bin/ember-applet"
pkg ember-applet "$ARCH" "$COMMON, $LIBS" "pulseaudio-utils, network-manager, bluez, rfkill" "Ember panel applet engine for COSMIC" \
    "The program behind Ember's COSMIC panel applets. Install the ember-applet-* packages for the applets themselves."

# one package per applet: just its launcher
declare -A FILE=([readings]="io.github.n1th1n_19.Ember.Applet" [clock]=Clock [sound]=Sound [bluetooth]=Bluetooth [network]=Network [power]=Power
    [dnd]=Dnd [tiling]=Tiling [session]=Session [media]=Media [awake]=Awake)
for a in "${APPLETS[@]}"; do
    f=${FILE[$a]}
    [[ "$f" == io.* ]] || f="io.github.n1th1n_19.Ember.Applet.$f"
    R="$STAGE/ember-applet-$a"
    install -Dm644 "res/applications/$f.desktop" "$R/usr/share/applications/$f.desktop"
    desc=$(sed -n 's/^Comment=//p' "res/applications/$f.desktop")
    pkg "ember-applet-$a" all "ember-applet (= $VERSION)" "" "Ember $a applet for the COSMIC panel" \
        "$desc. Add it to the top panel with: ember-applet --setup-panel $a"
done

# ember-full (metapackage)
mkdir -p "$STAGE/ember-full"
deps="ember (= $VERSION), ember-service (= $VERSION), ember-widget (= $VERSION)"
for a in "${APPLETS[@]}"; do deps="$deps, ember-applet-$a (= $VERSION)"; done
pkg ember-full all "$deps" "" "Ember: app, service, desktop widget and all panel applets" \
    "Installs every Ember component. Each one can also be installed on its own."
