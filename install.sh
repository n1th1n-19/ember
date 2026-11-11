#!/usr/bin/env bash
# Ember installer: install, update or remove Ember, all of it or just some parts.
#
#   curl -fsSL https://raw.githubusercontent.com/n1th1n-19/ember/main/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/n1th1n-19/ember/main/install.sh | bash -s -- --update
#   curl -fsSL https://raw.githubusercontent.com/n1th1n-19/ember/main/install.sh | bash -s -- --uninstall
#
# Pick parts with --only (comma-separated), e.g. --only app,widget,sound,clock
#   app       the Ember window (monitor, controls, apps manager, Quick panel)
#   service   history + alerts in the background
#   widget    desktop widget
#   applets   all panel applets, or name them: readings clock sound bluetooth network
#             power dnd tiling session media awake
# Without --only you're asked about each part. Also: --yes, --dry-run, --deb DIR|FILE.
# Once installed: ember-install --update | --uninstall [--only ...]
#
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

REPO="n1th1n-19/ember"
ID="io.github.n1th1n_19.Ember"
APPLETS=(readings clock sound bluetooth network power dnd tiling session media awake)
BINS=(ember ember-applet ember-widget emberd)

MODE=install YES=0 DRY=0 DEB="" ONLY=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --update) MODE=update ;;
        --uninstall) MODE=uninstall ;;
        --only) ONLY="$2"; shift ;;
        --yes | -y) YES=1 ;;
        --dry-run) DRY=1 ;;
        --deb) DEB="$2"; shift ;;
        -h | --help) sed -n '2,17p' "$0" 2>/dev/null || true; exit 0 ;;
        *) echo "Unknown option: $1" >&2; exit 2 ;;
    esac
    shift
done

if [[ -t 1 ]]; then B=$'\e[1m' D=$'\e[2m' Y=$'\e[33m' R=$'\e[31m' G=$'\e[32m' N=$'\e[0m'; else B="" D="" Y="" R="" G="" N=""; fi
say() { printf '%s\n' "$*"; }
step() { printf '%s==>%s %s\n' "$Y" "$N" "$*"; }
die() { printf '%serror:%s %s\n' "$R" "$N" "$*" >&2; exit 1; }
run() { if ((DRY)); then printf '%s  would run:%s %s\n' "$D" "$N" "$*"; else "$@"; fi; }
# yes/no question on the terminal, even when piped into bash. $2 = default (y|n)
ask() {
    local def=${2:-n} a
    ((YES)) && return 0
    if ((DRY)); then say "  ${D}(dry run: answering '$def') $1${N}"; [[ $def == y ]]; return; fi
    [[ -r /dev/tty ]] || die "no terminal to ask on; use --yes or --only"
    if [[ $def == y ]]; then read -r -p "$1 [Y/n] " a < /dev/tty; [[ ! "$a" == [nN]* ]]; else read -r -p "$1 [y/N] " a < /dev/tty; [[ "$a" == [yY]* ]]; fi
}

CONF="${XDG_CONFIG_HOME:-$HOME/.config}"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
STATE="${XDG_STATE_HOME:-$HOME/.local/state}"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}"
USER_PATHS=("$CONF/cosmic/$ID" "$CONF/autostart/$ID.Widget.desktop" "$STATE/ember" "$CACHE/ember" "$DATA/ember")
# Paths renamed or dropped in past versions; --update deletes them. Add here, never remove.
OBSOLETE=()

dev_paths() { # files a developer build put in ~/.local (they'd shadow the packages)
    local p
    for p in "${BINS[@]}"; do [[ -e "$HOME/.local/bin/$p" ]] && echo "$HOME/.local/bin/$p"; done
    compgen -G "$DATA/applications/$ID*.desktop" || true
    compgen -G "$DATA/icons/hicolor/*/apps/$ID.png" || true
}

need() { command -v "$1" >/dev/null 2>&1 || die "$1 is required"; }
arch() { dpkg --print-architecture; }
installed() { dpkg-query -W -f='${Status}' "$1" 2>/dev/null | grep -q "install ok installed"; }
installed_pkgs() { dpkg-query -W -f='${Package} ${Status}\n' 'ember*' 2>/dev/null | awk '/install ok installed/ {print $1}'; }
is_applet() { local a; for a in "${APPLETS[@]}"; do [[ $a == "$1" ]] && return 0; done; return 1; }
join() { local IFS=,; echo "$*"; }

# ---------- choosing parts ----------

SEL_APP=0 SEL_SERVICE=0 SEL_WIDGET=0 SEL_APPLETS=()
parse_only() {
    local part
    IFS=, read -ra parts <<< "$ONLY"
    for part in "${parts[@]}"; do
        part=${part// /}
        case "$part" in
            app) SEL_APP=1 ;;
            service) SEL_SERVICE=1 ;;
            widget) SEL_WIDGET=1 ;;
            applets) SEL_APPLETS=("${APPLETS[@]}") ;;
            all) SEL_APP=1 SEL_SERVICE=1 SEL_WIDGET=1 SEL_APPLETS=("${APPLETS[@]}") ;;
            "") ;;
            *) is_applet "$part" || die "unknown part '$part' (app service widget applets ${APPLETS[*]})"; SEL_APPLETS+=("$part") ;;
        esac
    done
}

choose_parts() {
    if [[ -n "$ONLY" ]]; then parse_only; return; fi
    if ((YES)); then SEL_APP=1 SEL_SERVICE=1 SEL_WIDGET=1 SEL_APPLETS=("${APPLETS[@]}"); return; fi
    say "${B}Choose what to install${N} (each part works on its own):"
    ask "  The Ember app (monitor, controls, apps manager, Quick panel)?" y && SEL_APP=1
    ask "  Background history + alerts service?" y && SEL_SERVICE=1
    ask "  Desktop widget?" y && SEL_WIDGET=1
    if ask "  All panel applets (${APPLETS[*]})?" y; then
        SEL_APPLETS=("${APPLETS[@]}")
    elif ask "  Pick panel applets one by one?" n; then
        local a
        for a in "${APPLETS[@]}"; do ask "    $a?" n && SEL_APPLETS+=("$a"); done
    fi
}

selected_pkgs() {
    local a
    ((SEL_APP)) && echo ember
    ((SEL_SERVICE)) && echo ember-service
    ((SEL_WIDGET)) && echo ember-widget
    for a in "${SEL_APPLETS[@]}"; do echo "ember-applet-$a"; done
    if ((SEL_APP || SEL_SERVICE || SEL_WIDGET || ${#SEL_APPLETS[@]})); then echo ember-common; fi
    ((${#SEL_APPLETS[@]})) && echo ember-applet
    return 0
}

# ---------- getting packages ----------

latest_version() {
    need curl
    local tag
    tag=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep -o '"tag_name": *"[^"]*"' | head -1 | sed 's/.*"v\{0,1\}\([^"]*\)"$/\1/') ||
        die "couldn't reach GitHub releases"
    [[ -n "$tag" ]] || die "no release found"
    echo "$tag"
}

# fetch_debs <version> <pkg>...  -> fills DEB_FILES
fetch_debs() {
    local ver=$1 p f
    shift
    DEB_FILES=()
    if [[ -n "$DEB" ]]; then
        local dir=$DEB
        [[ -f "$DEB" ]] && dir=$(dirname "$DEB")
        for p in "$@"; do
            f=$(compgen -G "$dir/${p}_*_$(arch).deb" || compgen -G "$dir/${p}_*_all.deb" || true)
            [[ -n "$f" ]] || die "no $p package in $dir"
            DEB_FILES+=("$(realpath "$(head -1 <<< "$f")")")
        done
        return
    fi
    TMP=$(mktemp -d)
    trap 'rm -rf "$TMP"' EXIT
    step "Downloading Ember $ver"
    for p in "$@"; do
        local a=$(arch)
        [[ $p == ember-common || $p == ember-full || $p == ember-applet-* ]] && a=all
        f="$TMP/${p}_${ver}_$a.deb"
        run curl -fsSL -o "$f" "https://github.com/$REPO/releases/download/v$ver/${p}_${ver}_$a.deb" || die "download failed: $p"
        DEB_FILES+=("$f")
    done
}

restart_panel() {
    pgrep -x cosmic-panel >/dev/null || return 0
    step "Restarting the COSMIC panel so applets reload (it comes straight back)"
    run pkill -x cosmic-panel || true
}

restart_stale() { # running copies from an old or developer build
    if pgrep -x ember-widget >/dev/null && [[ -x /usr/bin/ember-widget ]]; then
        run pkill -x ember-widget || true
        ((DRY)) || (sleep 1; setsid /usr/bin/ember-widget >/dev/null 2>&1 < /dev/null &)
    fi
    pgrep -x ember-applet >/dev/null && restart_panel
    return 0
}

remove_dev_copies() { # only the parts being installed now (SEL_*); everything on update
    local all=${1:-0}
    mapfile -t dev < <(dev_paths | while read -r f; do
        case "$f" in
            */bin/ember | */applications/$ID.desktop) ((all || SEL_APP)) && echo "$f" ;;
            */bin/emberd) ((all || SEL_SERVICE)) && echo "$f" ;;
            */bin/ember-widget | */applications/$ID.Widget.desktop) ((all || SEL_WIDGET)) && echo "$f" ;;
            */bin/ember-applet | */applications/$ID.Applet*.desktop) ((all || ${#SEL_APPLETS[@]})) && echo "$f" ;;
            *) echo "$f" ;; # icons: the package ships them
        esac
    done)
    ((${#dev[@]})) || return 0
    step "Removing developer-build copies that would shadow the packages"
    printf '  %s\n' "${dev[@]}"
    run rm -f "${dev[@]}"
}

restore_hidden_launchers() {
    local f name
    for f in "$DATA"/applications/*.desktop; do
        [[ -f "$f" ]] && grep -qx 'X-Ember-Hidden=true' "$f" || continue
        name=$(basename "$f")
        if compgen -G "/usr/share/applications/$name" >/dev/null || compgen -G "/var/lib/flatpak/exports/share/applications/$name" >/dev/null ||
            compgen -G "/var/lib/snapd/desktop/applications/$name" >/dev/null || compgen -G "$DATA/flatpak/exports/share/applications/$name" >/dev/null; then
            run rm -f "$f"
        else
            run sed -i '/^X-Ember-Hidden=true$/d;/^NoDisplay=true$/d' "$f"
        fi
        say "  restored launcher: $name"
    done
}

# ---------- install ----------

do_install() {
    need dpkg
    need apt-get
    choose_parts
    mapfile -t PKGS < <(selected_pkgs)
    ((${#PKGS[@]})) || { say "Nothing selected."; exit 0; }

    say ""
    say "${B}Will install these packages (needs your password):${N} ${PKGS[*]}"
    ((SEL_APP)) && say "  /usr/bin/ember, /usr/libexec/ember/ember-helper (root helper, only via a polkit prompt), polkit policy, app launcher"
    ((SEL_SERVICE)) && say "  /usr/bin/emberd, /usr/lib/systemd/user/ember.service"
    ((SEL_WIDGET)) && say "  /usr/bin/ember-widget, widget launcher"
    ((${#SEL_APPLETS[@]})) && say "  /usr/bin/ember-applet, launchers for: ${SEL_APPLETS[*]}"
    say "  ember-common: icon, licenses, /usr/bin/ember-install"
    say "  apt may also pull: libxkbcommon0 libwayland-client0 libegl1 libvulkan1 pkexec"
    say "${B}In your account, asked separately:${N} start the service, add applets to the top panel (COSMIC's"
    say "  matching applets swap out; ember-install --uninstall puts them back), widget at login."
    say "${B}Created while you use it:${N} $CONF/cosmic/$ID/  $STATE/ember/  $CACHE/ember/"
    ask "Continue?" y || { say "Nothing changed."; exit 0; }

    local ver=""
    [[ -n "$DEB" ]] || ver=$(latest_version)
    fetch_debs "$ver" "${PKGS[@]}"
    step "Installing (sudo)"
    run sudo apt-get install -y "${DEB_FILES[@]}"
    remove_dev_copies
    restart_stale

    if ((SEL_SERVICE)) && ask "Start the history + alerts service now and at login?" y; then
        run systemctl --user daemon-reload
        run systemctl --user enable --now ember.service
    fi
    if ((${#SEL_APPLETS[@]})) && pgrep -x cosmic-panel >/dev/null && ask "Add ${SEL_APPLETS[*]} to your top panel now?" y; then
        run /usr/bin/ember-applet --setup-panel "$(join "${SEL_APPLETS[@]}")"
    fi
    if ((SEL_WIDGET)) && ask "Show the desktop widget now and at every login?" y; then
        run /usr/bin/ember-widget --autostart on
        ((DRY)) || pgrep -x ember-widget >/dev/null || (setsid /usr/bin/ember-widget >/dev/null 2>&1 < /dev/null &)
    fi
    say "${G}Done.${N}  Add parts later: ember-install --only <parts>   Update: ember-install --update   Remove: ember-install --uninstall"
}

# ---------- update ----------

do_update() {
    need dpkg
    mapfile -t PKGS < <(installed_pkgs)
    ((${#PKGS[@]})) || die "Ember isn't installed (run without --update)"
    local cur ver
    cur=$(dpkg-query -W -f='${Version}' ember-common 2>/dev/null || true)
    if [[ -n "$DEB" ]]; then
        ver=""
    else
        ver=$(latest_version)
        [[ "$ver" == "$cur" ]] && { say "Ember $cur is up to date."; exit 0; }
    fi
    say "Update ${cur:-?} -> ${ver:-local packages}: ${PKGS[*]}"
    ask "Continue?" y || { say "Nothing changed."; exit 0; }
    fetch_debs "$ver" "${PKGS[@]}"
    step "Installing (sudo); dpkg removes files a new version no longer ships"
    run sudo apt-get install -y "${DEB_FILES[@]}"

    local p removed=0
    for p in "${OBSOLETE[@]}"; do
        p=${p/#\~/$HOME}
        [[ -e "$p" ]] || continue
        say "  removing obsolete: $p"
        run rm -rf -- "$p"
        removed=1
    done
    ((removed)) || say "  no obsolete files to remove"
    remove_dev_copies 1
    systemctl --user is-active --quiet ember.service 2>/dev/null && { step "Restarting ember.service"; run systemctl --user restart ember.service; }
    restart_stale
    say "${G}Updated.${N}"
}

# ---------- uninstall ----------

# remove_parts: uses SEL_* (what to remove). Full removal also wipes your Ember files.
do_uninstall() {
    local full=0 a
    if [[ -z "$ONLY" ]]; then full=1; else parse_only; fi
    mapfile -t have < <(installed_pkgs)

    local rm_pkgs=()
    if ((full)); then
        rm_pkgs=("${have[@]}")
    else
        ((SEL_APP)) && installed ember && rm_pkgs+=(ember)
        ((SEL_SERVICE)) && installed ember-service && rm_pkgs+=(ember-service)
        ((SEL_WIDGET)) && installed ember-widget && rm_pkgs+=(ember-widget)
        for a in "${SEL_APPLETS[@]}"; do installed "ember-applet-$a" && rm_pkgs+=("ember-applet-$a"); done
        installed ember-full && ((${#rm_pkgs[@]})) && rm_pkgs+=(ember-full) # the metapackage can't stay with parts missing
        # drop shared packages nothing else needs any more
        local left=() p
        for p in "${have[@]}"; do [[ " ${rm_pkgs[*]} " == *" $p "* ]] || left+=("$p"); done
        [[ " ${left[*]} " == *" ember-applet-"* ]] || { installed ember-applet && rm_pkgs+=(ember-applet); }
        local others=0
        for p in "${left[@]}"; do [[ $p == ember-common || $p == ember-applet ]] || others=1; done
        ((others)) || { installed ember-common && rm_pkgs+=(ember-common) && full=1; }
    fi
    mapfile -t dev < <(dev_paths)

    say "${B}This removes:${N}"
    ((${#rm_pkgs[@]})) && say "  packages: ${rm_pkgs[*]}"
    if ((full)); then
        say "  puts COSMIC's own panel applets back, restores launchers you hid with Ember,"
        say "  stops Ember, and deletes your Ember settings, history and caches:"
        printf '      %s\n' "${USER_PATHS[@]}"
        ((${#dev[@]})) && { say "  developer-build copies:"; printf '      %s\n' "${dev[@]}"; }
    fi
    ((${#rm_pkgs[@]} || full)) || { say "None of those parts are installed."; exit 0; }
    ask "Remove?" n || { say "Nothing changed."; exit 0; }

    local applet_bin="" e
    for e in /usr/bin/ember-applet "$HOME/.local/bin/ember-applet"; do [[ -x "$e" ]] && { applet_bin=$e; break; }; done
    local take_out=()
    if ((full)); then take_out=("${APPLETS[@]}"); else
        for p in "${rm_pkgs[@]}"; do [[ $p == ember-applet-* ]] && take_out+=("${p#ember-applet-}"); done
    fi
    if ((${#take_out[@]})); then
        step "Taking applets out of the panel: ${take_out[*]}"
        if [[ -n "$applet_bin" ]]; then
            run "$applet_bin" --restore-panel "$(join "${take_out[@]}")" || true
        elif ((full)) && [[ -f "$STATE/ember/panel-backup-plugins_wings.ron" ]]; then
            run cp "$STATE/ember/panel-backup-plugins_wings.ron" "$CONF/cosmic/com.system76.CosmicPanel.Panel/v1/plugins_wings"
        fi
    fi
    if ((full)) || [[ " ${rm_pkgs[*]} " == *" ember-widget "* ]]; then
        run rm -f "$CONF/autostart/$ID.Widget.desktop"
        run pkill -x ember-widget 2>/dev/null || true
    fi
    if ((full)) || [[ " ${rm_pkgs[*]} " == *" ember-service "* ]]; then
        run systemctl --user disable --now ember.service 2>/dev/null || true
    fi
    if ((full)) || [[ " ${rm_pkgs[*]} " == *" ember "* ]]; then
        step "Restoring launchers hidden with Ember"
        restore_hidden_launchers
        run pkill -x ember 2>/dev/null || true
    fi
    ((full)) && { run pkill -x emberd 2>/dev/null || true; }

    if ((${#rm_pkgs[@]})); then
        step "Removing packages (sudo)"
        run sudo apt-get purge -y "${rm_pkgs[@]}"
    fi
    if ((full)); then
        step "Deleting your Ember files"
        for p in "${USER_PATHS[@]}" "${dev[@]}"; do [[ -e "$p" ]] && { say "  $p"; run rm -rf -- "$p"; }; done
        run systemctl --user daemon-reload 2>/dev/null || true
    fi
    restart_panel

    ((DRY)) && { say "${G}Dry run finished.${N}"; return; }
    ((full)) || { say "${G}Removed.${N}"; return; }
    local left
    left=$(find "$CONF" "$DATA" "$STATE" "$CACHE" "$HOME/.local/bin" /usr/bin /usr/libexec /usr/share/applications /usr/share/icons /usr/share/polkit-1 \
        /usr/lib/systemd/user /usr/share/metainfo /usr/share/doc \( -name "$ID*" -o -name 'ember' -o -name 'ember-*' -o -name 'emberd' -o -name 'ember.service' \) \
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
