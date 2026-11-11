#!/usr/bin/env bash
# Ember installer: install, update or completely uninstall.
#
#   curl -fsSL https://raw.githubusercontent.com/n1th1n-19/ember/main/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/n1th1n-19/ember/main/install.sh | bash -s -- --update
#   curl -fsSL https://raw.githubusercontent.com/n1th1n-19/ember/main/install.sh | bash -s -- --uninstall
#
# Once installed it is also available as `ember-install --update` / `--uninstall`.
# Options: --yes (don't ask), --dry-run (print what would happen), --deb FILE (use a local package).
#
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

REPO="n1th1n-19/ember"
ID="io.github.n1th1n_19.Ember"
BINS=(ember ember-applet ember-widget emberd)

MODE=install YES=0 DRY=0 DEB=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --update) MODE=update ;;
        --uninstall) MODE=uninstall ;;
        --yes | -y) YES=1 ;;
        --dry-run) DRY=1 ;;
        --deb) DEB="$2"; shift ;;
        -h | --help) sed -n '2,12p' "$0" 2>/dev/null || true; exit 0 ;;
        *) echo "Unknown option: $1" >&2; exit 2 ;;
    esac
    shift
done

if [[ -t 1 ]]; then B=$'\e[1m' D=$'\e[2m' Y=$'\e[33m' R=$'\e[31m' G=$'\e[32m' N=$'\e[0m'; else B="" D="" Y="" R="" G="" N=""; fi
say() { printf '%s\n' "$*"; }
step() { printf '%s==>%s %s\n' "$Y" "$N" "$*"; }
die() { printf '%serror:%s %s\n' "$R" "$N" "$*" >&2; exit 1; }
# run a command, or only print it with --dry-run
run() { if ((DRY)); then printf '%s  would run:%s %s\n' "$D" "$N" "$*"; else "$@"; fi; }
# yes/no question; reads the terminal even when this script is piped into bash
ask() {
    ((YES)) && return 0
    ((DRY)) && { say "  ${D}(dry run: would ask) $1${N}"; return 0; }
    local a
    [[ -r /dev/tty ]] || die "no terminal to ask on; re-run with --yes"
    read -r -p "$1 [y/N] " a < /dev/tty
    [[ "$a" == [yY]* ]]
}

CONF="${XDG_CONFIG_HOME:-$HOME/.config}"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
STATE="${XDG_STATE_HOME:-$HOME/.local/state}"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}"

# Paths Ember writes in your home folder (removed by --uninstall).
USER_PATHS=(
    "$CONF/cosmic/$ID"
    "$CONF/autostart/$ID.Widget.desktop"
    "$STATE/ember"
    "$CACHE/ember"
    "$DATA/ember"
)
# Files a developer build may have put in ~/.local (shadow the packaged ones).
dev_paths() {
    local p
    for p in "${BINS[@]}"; do [[ -e "$HOME/.local/bin/$p" ]] && echo "$HOME/.local/bin/$p"; done
    compgen -G "$DATA/applications/$ID*.desktop" || true
    compgen -G "$DATA/icons/hicolor/*/apps/$ID.png" || true
}
# Paths renamed or dropped in past versions; --update deletes them. Add here, never remove.
OBSOLETE=()

need() { command -v "$1" >/dev/null 2>&1 || die "$1 is required"; }
arch() { dpkg --print-architecture; }
installed_version() { dpkg-query -W -f='${Version}' ember 2>/dev/null || true; }

latest_release() { # prints "<version> <url>"
    need curl
    local json url
    json=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest") || die "couldn't reach GitHub releases"
    url=$(printf '%s' "$json" | grep -o "\"browser_download_url\": *\"[^\"]*_$(arch)\\.deb\"" | head -1 | sed 's/.*"\(http[^"]*\)"/\1/')
    [[ -n "$url" ]] || die "no .deb for $(arch) in the latest release"
    local ver=${url##*/ember_}
    printf '%s %s\n' "${ver%%_*}" "$url"
}

fetch_deb() { # sets DEB_FILE (and DEB_VERSION when known)
    if [[ -n "$DEB" ]]; then
        [[ -f "$DEB" ]] || die "no such file: $DEB"
        DEB_FILE=$(realpath "$DEB")
        DEB_VERSION=$(dpkg-deb -f "$DEB_FILE" Version)
        return
    fi
    read -r DEB_VERSION url < <(latest_release)
    TMP=$(mktemp -d)
    trap 'rm -rf "$TMP"' EXIT
    DEB_FILE="$TMP/ember_${DEB_VERSION}_$(arch).deb"
    step "Downloading Ember $DEB_VERSION"
    run curl -fL --progress-bar -o "$DEB_FILE" "$url"
}

restart_panel() {
    pgrep -x cosmic-panel >/dev/null || return 0
    step "Restarting the COSMIC panel so applets reload (it comes straight back)"
    run pkill -x cosmic-panel || true
}

restore_hidden_launchers() {
    # Ember's "hide from launcher" marks what it changes; undo exactly those.
    local f name
    for f in "$DATA"/applications/*.desktop; do
        [[ -f "$f" ]] && grep -qx 'X-Ember-Hidden=true' "$f" || continue
        name=$(basename "$f")
        if compgen -G "/usr/share/applications/$name" >/dev/null || compgen -G "/var/lib/flatpak/exports/share/applications/$name" >/dev/null ||
            compgen -G "/var/lib/snapd/desktop/applications/$name" >/dev/null || compgen -G "$DATA/flatpak/exports/share/applications/$name" >/dev/null; then
            run rm -f "$f"          # our override copy of a system launcher
        else
            run sed -i '/^X-Ember-Hidden=true$/d;/^NoDisplay=true$/d' "$f"   # edited in place
        fi
        say "  restored launcher: $name"
    done
}

print_install_plan() {
    say "${B}Ember will install:${N}"
    say "  ${B}System (package 'ember', needs your password):${N}"
    say "    /usr/bin/ember  ember-applet  ember-widget  emberd  ember-install"
    say "    /usr/libexec/ember/ember-helper            (root helper, run only through a polkit prompt)"
    say "    /usr/share/applications/$ID*.desktop        (app, 11 panel applets, widget)"
    say "    /usr/share/icons/hicolor/*/apps/$ID.png"
    say "    /usr/share/polkit-1/actions/$ID.helper.policy"
    say "    /usr/lib/systemd/user/ember.service        (history + alerts)"
    say "    /usr/share/metainfo/$ID.metainfo.xml,  /usr/share/doc/ember/"
    say "    apt may also pull: libxkbcommon0 libwayland-client0 libegl1 libvulkan1 pkexec"
    say "  ${B}Your account (each asked separately):${N}"
    say "    enable ember.service for your user"
    say "    swap COSMIC's top-panel applets for Ember's   (backup kept; ember --restore-panel undoes)"
    say "    $CONF/autostart/$ID.Widget.desktop          (desktop widget at login)"
    say "  ${B}Created while you use it:${N}"
    say "    $CONF/cosmic/$ID/   $STATE/ember/   $CACHE/ember/"
}

do_install() {
    need dpkg
    need apt-get
    local cur
    cur=$(installed_version)
    [[ -n "$cur" && -z "$DEB" ]] && say "Ember $cur is already installed; use --update to upgrade."
    print_install_plan
    ask "Continue?" || { say "Nothing changed."; exit 0; }
    fetch_deb
    step "Installing the package (sudo)"
    run sudo apt-get install -y "$DEB_FILE"

    mapfile -t dev < <(dev_paths)
    if ((${#dev[@]})); then
        step "Removing developer-build copies that would shadow the package"
        printf '  %s\n' "${dev[@]}"
        run rm -f "${dev[@]}"
    fi

    # anything already running from a developer build points at deleted files: restart it
    if pgrep -x ember-widget >/dev/null; then
        run pkill -x ember-widget || true
        ((DRY)) || (sleep 1; setsid /usr/bin/ember-widget >/dev/null 2>&1 < /dev/null &)
    fi
    pgrep -f '/ember-applet' >/dev/null && restart_panel

    if ask "Run Ember's history + alerts service for your user?"; then
        run systemctl --user daemon-reload
        run systemctl --user enable --now ember.service
    fi
    if pgrep -x cosmic-panel >/dev/null && ask "Use Ember's applets in the top panel (replaces COSMIC's, same spots)?"; then
        run /usr/bin/ember --setup-panel
    fi
    if ask "Show the desktop widget now and at every login?"; then
        run /usr/bin/ember --widget-autostart on
        ((DRY)) || (setsid /usr/bin/ember-widget >/dev/null 2>&1 < /dev/null &)
    fi
    say "${G}Done.${N} Open Ember from your app list. Update: ember-install --update   Remove: ember-install --uninstall"
}

do_update() {
    need dpkg
    local cur
    cur=$(installed_version)
    [[ -n "$cur" ]] || die "Ember isn't installed (run without --update)"
    if [[ -z "$DEB" ]]; then
        local latest
        read -r latest _ < <(latest_release)
        if [[ "$latest" == "$cur" ]]; then say "Ember $cur is up to date."; exit 0; fi
        say "Update Ember $cur -> $latest"
    fi
    ask "Continue?" || { say "Nothing changed."; exit 0; }
    fetch_deb
    step "Installing $DEB_VERSION (sudo); dpkg removes files the new version no longer ships"
    run sudo apt-get install -y "$DEB_FILE"

    local p removed=0
    for p in "${OBSOLETE[@]}"; do
        p=${p/#\~/$HOME}
        [[ -e "$p" ]] || continue
        say "  removing obsolete: $p"
        run rm -rf -- "$p"
        removed=1
    done
    ((removed)) || say "  no obsolete files to remove"
    mapfile -t dev < <(dev_paths)
    ((${#dev[@]})) && { say "  removing developer-build copies:"; printf '    %s\n' "${dev[@]}"; run rm -f "${dev[@]}"; }

    systemctl --user is-active --quiet ember.service 2>/dev/null && { step "Restarting ember.service"; run systemctl --user restart ember.service; }
    if pgrep -x ember-widget >/dev/null; then
        step "Restarting the desktop widget"
        run pkill -x ember-widget || true
        ((DRY)) || (sleep 1; setsid /usr/bin/ember-widget >/dev/null 2>&1 < /dev/null &)
    fi
    pgrep -f '/ember-applet' >/dev/null && restart_panel
    say "${G}Updated to $DEB_VERSION.${N}"
}

do_uninstall() {
    say "${B}This removes Ember completely:${N}"
    say "  - puts COSMIC's own panel applets back and removes Ember's"
    say "  - restores app launchers you hid with Ember"
    say "  - stops Ember's service, widget, applets and windows"
    say "  - removes the package 'ember' and everything it installed (sudo)"
    say "  - deletes your Ember settings, history and caches:"
    printf '      %s\n' "${USER_PATHS[@]}"
    mapfile -t dev < <(dev_paths)
    ((${#dev[@]})) && { say "  - deletes developer-build copies:"; printf '      %s\n' "${dev[@]}"; }
    ask "Remove Ember?" || { say "Nothing changed."; exit 0; }

    local ember=""
    for e in /usr/bin/ember "$HOME/.local/bin/ember"; do [[ -x "$e" ]] && { ember=$e; break; }; done
    if [[ -n "$ember" ]]; then
        step "Restoring COSMIC's panel applets"
        run "$ember" --restore-panel || true
        run "$ember" --widget-autostart off || true
    elif [[ -f "$STATE/ember/panel-backup-plugins_wings.ron" ]]; then
        say "  ${Y}ember binary missing; restoring the panel layout saved before Ember${N}"
        run cp "$STATE/ember/panel-backup-plugins_wings.ron" "$CONF/cosmic/com.system76.CosmicPanel.Panel/v1/plugins_wings"
    fi
    step "Restoring hidden app launchers"
    restore_hidden_launchers

    step "Stopping Ember"
    run systemctl --user disable --now ember.service 2>/dev/null || true
    for b in "${BINS[@]}"; do run pkill -x "$b" 2>/dev/null || true; done

    if [[ -n "$(installed_version)" ]]; then
        step "Removing the package (sudo)"
        run sudo apt-get purge -y ember
    fi
    step "Deleting your Ember files"
    for p in "${USER_PATHS[@]}" "${dev[@]}"; do
        [[ -e "$p" ]] && { say "  $p"; run rm -rf -- "$p"; }
    done
    run systemctl --user daemon-reload 2>/dev/null || true
    restart_panel

    # Final check: anything still named after Ember in the places it ever writes.
    ((DRY)) && { say "${G}Dry run finished.${N}"; return; }
    local left
    left=$(find "$CONF" "$DATA" "$STATE" "$CACHE" "$HOME/.local/bin" /usr/bin /usr/libexec /usr/share/applications /usr/share/icons /usr/share/polkit-1 /usr/lib/systemd/user /usr/share/metainfo /usr/share/doc \
        \( -name "$ID*" -o -name 'ember' -o -name 'ember-applet' -o -name 'ember-widget' -o -name 'emberd' -o -name 'ember-helper' -o -name 'ember-install' -o -name 'ember.service' \) \
        -not -path "*/Documents/*" 2>/dev/null || true)
    if [[ -n "$left" ]]; then
        say "${Y}Still present (not removed automatically):${N}"
        printf '  %s\n' "$left"
    else
        say "${G}Ember is completely removed.${N}"
    fi
}

case "$MODE" in
    install) do_install ;;
    update) do_update ;;
    uninstall) do_uninstall ;;
esac
