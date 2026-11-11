# Changelog

## Unreleased

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
