# Ember design

Comic-book print style for a native COSMIC app. Everything is drawn in code
(iced `canvas` paths), so it scales cleanly and needs no image assets.

## Tokens
| Token | Value | Use |
|---|---|---|
| ink | `#111111` | outlines, text on light, hard shadows |
| paper | `#F6EFE0` | page ground, text on ink |
| red | `#E8202A` | danger, hot, CPU |
| magenta | `#FF2E88` | memory, misprint right |
| cyan | `#00C2FF` | network, cool, misprint left |
| yellow | `#FFD400` | captions, highlights, battery charge |
| white | `#FFFFFF` | speech bubbles, chart paper |

- Outlines 3px (small) / 5px (panels). Shadows offset 6/4/3px, **no blur**.
- Fonts: Bangers (display, numbers), Permanent Marker (bubbles, notes), Space Grotesk (body, labels).
- Textures (halftone dots, hatch, crosshatch, speed lines) only on art and chart fills, never behind body text.
- No gradients, glows, blur or glass.

## Motion
- One pose = 83ms. Every animation snaps to whole poses (stepped, never eased).
- Loops: at most 2 per screen. Entrances play once.
- "Reduce motion" setting freezes all loops; art still shows the current state statically.
- Misprint (cyan left, magenta right offset) only on numbers 48px or bigger.
- SFX words are original: SIZZLE!, GLUG, FWOOSH, CLUNK, ZZZ, KRAK, PLINK.

## State levels (shared)
Every metric maps to **calm / busy / hot** and the art changes pose per level:
- calm: < 50% (or < 60 °C)
- busy: 50–85% (60–80 °C)
- hot: > 85% (> 80 °C), panel gets a red caption + one SFX burst on entry

## Panels

Each panel: art on the left (≈40%), live number + chart on the right, yellow
caption tag top-left with the panel name, details strip at the bottom.

### CPU — "Engine Room"
- **Art:** a boiler/furnace drawn in ink. Firebox flames height = total CPU %.
  A row of 12 pistons on top, one per core, each piston's height = that core's %.
  Pressure gauge on the boiler: needle = clock MHz vs max.
- **Animation:** flames flicker (3 poses, loop). Pistons step to the new height
  in 2 poses each sample. Hot: steam puffs + speed lines + "SIZZLE!" burst once.
- **Charts:** 60 s area sparkline (red, halftone fill); per-core bar strip;
  load average 1/5/15 as three stacked tick marks.
- **Details:** model name, MHz, load, uptime.

### Memory — "Ink Tank"
- **Art:** a glass tank with ink level = RAM %. A small bucket under an overflow
  pipe holds swap, level = swap %.
- **Animation:** ink surface wobbles (2 poses). Swap rising: drip falls from the
  pipe each sample + "GLUG". Hot: tank glass cracks (hatch lines).
- **Charts:** 60 s stacked area (used magenta, swap hatch); used/total donut.
- **Details:** used / total GB, swap used / total, available.

### Disks — "Filing Cabinet"
- **Art:** one cabinet drawer per mounted disk; drawer fill bar = used %.
- **Animation:** drawer slides out 1 notch while reading/writing, back when idle.
  Big write burst: "CLUNK".
- **Charts:** per-disk horizontal used bar; mirrored read/write column chart
  (60 s, read up cyan, write down red).
- **Details:** device, mount, used / total, read/s, write/s.

### Network — "Radio Tower"
- **Art:** a transmitter tower. Signal rings around the antenna; ring count =
  traffic level (0–3). Down arrow cyan, up arrow red with live speeds.
- **Animation:** rings pulse outward (4 poses) only while traffic > 0.
  Burst over 10 MB/s: "FWOOSH" + speed lines.
- **Charts:** 60 s mirrored area (download above axis, upload below).
- **Details:** rx/s, tx/s, session totals.

### Temperatures — "Thermometer Rack"
- **Art:** row of thermometers grouped as CPU / SSD / RAM / Board. Mercury
  height = °C vs that sensor's critical value (fallback 100 °C).
- **Animation:** mercury steps to new level in 2 poses. Hot: sweat drops on the
  thermometer, red caption "TOO HOT".
- **Charts:** heat strip (one cell per sensor, color stepped calm cyan → yellow
  → red); 60 s line of the hottest CPU sensor.
- **Details:** each sensor label, °C, critical.

### Battery — "Power Cell"
- **Art:** big battery cell; yellow halftone fill = charge %. Bolt icon when
  charging, plug when full on AC, nothing when discharging.
- **Animation:** charging: bolt flickers (2 poses) + fill creeps up 1 notch per
  pose. Under 15%: cell shakes once + "PLINK".
- **Charts:** health ring (full / design), 1 h charge line, power draw W gauge.
- **Details:** status, health %, cycles, watts.

### GPU — "Graphics Card"
- **Art:** one card per GPU. Card fan blades + clock dial (cur / max MHz).
  dGPU with no driver: card is drawn grey with sleeping eyes + "ZZZ" bubble,
  caption "Switched off (integrated mode)".
- **Animation:** fan blades spin (stepped 8 poses), speed by usage or clock.
- **Charts:** 60 s clock line; usage + VRAM bars when NVML provides them.
- **Details:** vendor, driver, clock, usage, VRAM.

### Fans — "Fan Box"
- **Art:** if fans exist, one fan per entry spinning by RPM.
  If none: a fan with tape across it, speech bubble "Firmware keeps these
  locked", fan control hidden.
- **Charts:** RPM line when present.

## Overview screen
Comic page of 6 small panels (CPU, Memory, GPU, Temps, Battery, Network) in a
grid with gutters, each showing its art at small size + big misprint number.
Click a panel to open its full page.

## Accessibility
- Every number also has a text label; art is decorative.
- Contrast: ink on paper / paper on ink only for text.
- Visible focus ring (cyan 4px) on every control, targets ≥ 44px.
