# Security

Ember has one part that runs as root: `ember-helper` (`/usr/libexec/ember/ember-helper`).
It only runs through `pkexec`, after a polkit password prompt, and accepts six fixed
commands (`governor`, `max-perf`, `turbo`, `renice`, `signal`, `pwm`). Every argument is
parsed into a typed value, file paths are built from fixed parts, no shell is used, and
PID 0/1 are refused. Its tests are in `crates/helper/src/main.rs`.

Everything else runs as you. System changes go through services that do their own
authorization (systemd, system76-power, NetworkManager, BlueZ, logind, PackageKit/apt via
pkexec). Wi-Fi passwords are sent to NetworkManager over D-Bus, never on a command line.

## Reporting a vulnerability

Please don't open a public issue. Use GitHub's private
[security advisory form](https://github.com/n1th1n-19/ember/security/advisories/new).
You'll get a reply within a week.
