# Changelog

## 0.1.4 — 2026-11-14

- Fan mode: Control page and the Power applet switch the firmware thermal mode
  (`platform_profile`: Quiet, Balanced, Performance, Extreme, Custom on Lenovo; whatever
  the laptop offers elsewhere).
- Custom mode: sliders for the firmware's CPU/GPU power and temperature limits
  (`/sys/class/firmware-attributes`, e.g. PL1/PL2, cTGP, thermal limits on Lenovo LOQ/Legion),
  bounded by the min/max the firmware reports. They apply in Custom mode.
- Fan mode and custom limits are saved to `/etc/ember/firmware.conf` and re-applied at boot
  by the new `ember-firmware.service` (enabled by the `ember` package, removed on purge).

## 0.1.3 — 2026-11-12

- Battery: when the charger is plugged in but the firmware holds the charge (Long Life mode
  or a charge limit), the page says so and shows "0 W · on charger" instead of a bare 0 W.
- Session: the restart/shut down confirmation puts its message on its own line, so the
  "Yes" and "Cancel" buttons are no longer squeezed into a thin red strip in the applet.
- Readings applet: shows each reading's number (and name, when the panel has room) next
  to its bar; on a small panel it used to show only unreadable slivers.

## 0.1.2 — 2026-11-12

- NVIDIA cards are read through NVIDIA's own driver library (NVML): usage, clock, temperature,
  power draw and video memory. Ember only asks while the card is awake, so it never wakes a
  sleeping dGPU.
- GPU page: each card gets its own chart (built-in GPU clock in cyan, NVIDIA usage in magenta)
  and its own line of readings. The overview shows NVIDIA usage while that card is working.
- History file format v2 (adds NVIDIA usage); the old file is replaced on first run.

## 0.1.1 — 2026-11-11

- `ember update`, `ember uninstall [parts]`, `ember install <parts>`, `ember version`, `ember help`.
- Installer: progress bars for downloads, install, update and uninstall; falls back to the saved
  panel layout if restoring the panel fails; refreshes COSMIC's launcher after installing the app.
- The window has a title ("Ember — CPU" …) instead of showing as "unknown" in the launcher.
- Copy buttons switch to "Copied!" for two seconds.

## 0.1.0 — 2026-11-11

First release.

- Comic-style monitor: overview plus CPU, memory, disks, network, temperatures, battery,
  GPU and fan pages, each with its own animated illustration and charts; dark and light themes.
- `emberd` service: one hour of history and desktop alerts (CPU, memory, CPU temperature,
  low battery).
- Controls: processes (end, kill, pause, priority), systemd services, power profile,
  graphics mode, battery charge limit, CPU governor, turbo and max performance, fans.
- Apps page: every installed app from APT, Flatpak, Snap, AppImage, launchers, cargo,
  npm and bun, with uninstall, hide, and the equivalent command.
- Quick panel and COSMIC panel applets: clock, sound, Bluetooth, network, power,
  Do Not Disturb, tiling, session, media, keep awake, live readings.
- Desktop widget with live placement from Settings.
- Separate packages for the app, service, widget and each panel applet, plus `ember-full`.
- Installer that installs, updates or removes all of Ember or just chosen parts.
