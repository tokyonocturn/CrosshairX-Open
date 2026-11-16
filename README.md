# CrosshairX-Open

A free, open-source replacement for paid crosshair overlay apps on Windows 10 and 11.

CrosshairX-Open draws a transparent, click-through, always-on-top crosshair on top of
your desktop and games. You design it in a live editor, it appears instantly, and it
never talks to the network.

![The designer](docs/screenshots/designer.png)

* Open source (MIT), no accounts, no ads, no subscriptions, no telemetry, works offline
* Transparent, click-through, always-on-top overlay (layered Win32 window)
* Custom shapes, colours, sizes, opacity, outlines, gaps and per-layer positioning
* Live editor whose preview is rendered by the same rasteriser as the overlay
* Local preset library with JSON import/export, including the community payload format
  used by the original Crosshair X
* Global hotkeys: toggle the overlay, nudge it pixel by pixel
* Multi-monitor, per-monitor DPI v2, any resolution
* Idle CPU use is effectively zero: the overlay only repaints when something changes

## Contents

| Page | What it does |
| --- | --- |
| Designer | Dot shape/size/colour, four arms with length, thickness, gap, rotation, per-arm visibility, outline, blur, layer offset and scale. |
| Presets | Save, overwrite, duplicate, delete, import and export designs as JSON. |
| Position & Size | Pick the display, offset from its centre, global size and opacity, DPI handling. |
| Settings | Toggle and position hotkeys, nudge step, start with Windows, data folder. |
| About | Version, how it works, links. |

More screenshots: [presets](docs/screenshots/presets-page.png),
[position & size](docs/screenshots/position.png), [settings](docs/screenshots/settings.png),
[about](docs/screenshots/about.png), and the bundled designs rendered by the engine in
[`docs/screenshots/presets/`](docs/screenshots/presets).

## Install

Download `crosshairx-open.exe` from the [latest release](../../releases/latest) and run it.
It is a single self-contained executable; no installer and no admin rights are needed.
Settings and presets live in `%APPDATA%\CrosshairX-Open`.

Windows SmartScreen may warn about an unsigned binary from a new publisher. That is
normal for a fresh open-source project; build it yourself if you prefer.

## Build from source

You need a Rust toolchain (1.88 or newer) and either the MSVC build tools or the
GNU toolchain (`rustup default stable-x86_64-pc-windows-msvc`, or
`rustup default stable-x86_64-pc-windows-gnu` with MSYS2's gcc on `PATH`).

```bash
git clone https://github.com/tokyonocturn/CrosshairX-Open.git
cd CrosshairX-Open
# target/release/crosshairx-open.exe
```

Development commands:

```bash
cargo test --workspace                 # 40 unit tests (geometry, presets, rasteriser)
cargo run                              # open the editor
cargo run -- --self-test               # end-to-end checks, including on-screen tests
cargo run -- --export-previews out/    # render the bundled presets to PNG
cargo run -- --ui-snapshot shot.png    # save a screenshot of the editor window
```

The release executable is built for the Windows GUI subsystem so that double-clicking it
never opens a console window. When you call a command-line flag from PowerShell, wait for
it explicitly (`Start-Process .\crosshairx-open.exe -ArgumentList "--export-previews","out" -Wait`),
because PowerShell does not wait for GUI-subsystem processes on its own. `cmd.exe` and
Git Bash wait normally.

## Using it

Start the app, pick a preset or design your own, and leave it running. The overlay is
click-through, so it never takes mouse input away from the game.

| Shortcut | Action |
| --- | --- |
| `Alt + Shift + Z` | Show/hide the crosshair (configurable in Settings) |
| `Alt + Shift + Arrow keys` | Move the crosshair one pixel (configurable) |

Both shortcuts are registered with `RegisterHotKey`, so they work while a game has
focus and no keyboard hooks are installed.

### Preset files

Presets are plain JSON. `Export current...` writes this project's format:

```json
{
  "format": "crosshairx-open/1",
  "name": "Wide Gap Green",
  "layers": [
    {
      "dot":  { "diameter": 2, "opacity": 1, "color": "#00FF88", "shape": "square", "thickness": 1, "blur": 0 },
      "line": { "length": 10, "thickness": 2, "offset": 6, "opacity": 1, "color": "#00FF88",
                "shape": "rectangle", "rotation": 0, "blur": 0,
                "visible": ["top", "bottom", "left", "right"] },
      "outline": { "thickness": 1, "opacity": 1, "color": "#000000", "blur": 0 },
      "position": { "x": 0, "y": 0 },
      "layer": { "size": 1 }
    }
  ]
}
```

`Import JSON...` also accepts the community payload shape
(`{"crosshairId": [ ... ], "name": "..."}`), arrays of those, and the
`{"items": [...]}` page wrapper, so designs exported from the original app load here.
Unknown fields (such as its `firingOptions`) are ignored rather than rejected.

## How it works

```
crates/crosshair-core   model + geometry + rasteriser + preset library (no OS code)
src/win32.rs            hand-written Win32 bindings (no binding crate)
src/overlay.rs          layered overlay window, message loop, hotkeys
src/ui.rs               egui editor
src/startup.rs          per-user Run key for "start with Windows"
src/selftest.rs         scriptable end-to-end checks
```

* The overlay is a `WS_POPUP` window with `WS_EX_LAYERED | WS_EX_TRANSPARENT |
  WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`, blitted with
  `UpdateLayeredWindow` from a 32-bit premultiplied DIB. Only the crosshair's own
  bounding box is covered, not the whole screen.
* `crosshair-core` renders into a premultiplied RGBA pixmap with `tiny-skia`, which is
  exactly what `UpdateLayeredWindow` expects, and the editor preview uploads the same
  pixmap as a texture. What you see in the designer is what the overlay draws.
* Per-monitor DPI v2 awareness; the crosshair is scaled by the selected display's DPI
  unless you tick "ignore display DPI scaling", in which case sizes are physical pixels.
* Nothing polls: the overlay thread blocks in `GetMessage`, and the editor requests a
  repaint only when you change something.

## Verification

`crosshairx-open --self-test` exercises the shipped binary, not a mock. On a machine
with three displays it printed:

```
[info] 3 display(s) detected
       Display 1 - 1920x1080 @ 96 DPI (primary) at (0, 0) ...
[ ok ] preset Classic Cross        30x30 px, 160 non-transparent px
...
[ ok ] preset export/import round-trip (1)
[ ok ] community payload import ('Community Sample', 1 layer(s))
[ ok ] overlay window created (hwnd 0x270936, class 'CrosshairXOpenOverlay')
[ ok ] the crosshair is composited on screen (+234 px)
[ ok ] window style has WS_EX_LAYERED
[ ok ] window style has WS_EX_TRANSPARENT (click-through)
[ ok ] window style has WS_EX_TOPMOST
[ ok ] window style has WS_EX_NOACTIVATE
[ ok ] clicks pass through (WindowFromPoint -> class 'UnityWndClass')
[ ok ] hiding the overlay removes it from the screen
[ ok ] offset moves the crosshair (+192 px at x+20)
[ ok ] display selection moves the crosshair to Display 2 - 1920x1080 @ 96 DPI
[ ok ] global hotkey Alt + Shift + Z toggled the overlay
[ ok ] hotkey hid the overlay from the screen
[ ok ] position hotkey moved the crosshair 1px (x 20 -> 21)
[ ok ] autostart entry written to HKCU Run and read back
[ ok ] autostart entry restored to its previous state (disabled)
[ ok ] stopping the overlay thread tears the window down

=== all checks passed ===
```

The on-screen checks capture the desktop before and after showing the overlay and count
pixels of the exact green the test crosshair uses, so "it renders" means "it is actually
composited on the screen", over whatever window happened to be in front.

`cargo test --workspace` covers the geometry (arm rectangles, gaps, outline frames,
triangle points, rotation, extent, image projection), colour parsing, preset
import/export/round-trip, settings persistence and corruption recovery, and pixel-level
rasteriser behaviour (opacity, hidden arms, dot shapes, hollow rings, blur bleed, layer
transform).

## Resource use

Measured on Windows 11 (16 logical cores, RTX 3060, 165 Hz panel), release build, with
the overlay visible. CPU is process CPU time sampled from `Get-Process`; GPU is the
per-process GPU Engine utilization counters summed over every engine instance belonging
to the app (18 instances were tracked, so a zero is a real zero rather than a missing
counter).

| Scenario | CPU | GPU |
| --- | --- | --- |
| Desktop idle, 180 s | 813 ms total, all of it in the first 15 s; 0.0 ms in each of the following 11 samples | 0.00% |
| Continuously rendering fullscreen 1080p60 video in the foreground, 60 s | **0.0 ms** in every 5 s sample | **0.00%** |
| Editor open, pointer resting over it, 12 s | **0.0 ms** | 0.00% |
| Editor actively used: slider dragged continuously, 20 s | ~5 s (26-30% of one core, ~1.7% of the machine) | 0-2% |

The overlay costs nothing while something else is drawing. Being a layered window,
Windows keeps its composited surface, and the app never repaints because the screen
changed underneath it. The same holds with the editor open and the pointer parked on it.

### Editing costs real CPU

Dragging a slider for 20 s costs about 5 s of CPU: **26-30% of one core**, roughly 1.7%
of a 16-core machine. That is not near zero, and it is the editor window, not the overlay:
egui repaints the whole window about 90-110 times a second while the pointer moves.

Measured inside the app with `--ui-stats`, the work that belongs to this code is small:

| Where a frame goes | Cost |
| --- | --- |
| `App::update` (widgets, preview raster, state commit) | 0.25 ms per frame |
| Overlay refresh, on its own thread | 0.15 ms per refresh, about 22 per second |
| egui layout and tessellation, GL draw, present | ~2.2 ms per frame at 60-110 fps |

What the measurement led to, and what it changed:

* `SetWindowPos` ran on every overlay refresh and cost 0.35-1.03 ms - more than the
  crosshair render and the blit combined. It now runs only when the crosshair rectangle
  changes, or after the overlay is shown again. Overlay-thread CPU during a drag fell
  from 6.1% to 3.3% of a core, and a refresh fell from 0.55-1.2 ms to 0.15 ms.
* The preview checkerboard was ~340 filled rectangles per frame; it is one tiled texture.
* The deferred preview render scheduled a frame of its own on top of the input frames.
  It now waits for the next input frame while the pointer is down: ~110 frames per second
  became ~93. Throttling the overlay's repaint requests as well was tried and dropped -
  it changed the frame count by less than the run-to-run noise, at the cost of delaying
  hotkey feedback by up to 50 ms.
* Preview rasterisation is capped at ~30 per second and settings save at most 3 times a
  second, both previously per frame.

### The cost is per frame, and frames come from input

Two drags with the same slider and different pointer event rates:

| Pointer events | Frames drawn | Editor CPU | CPU per frame |
| --- | --- | --- | --- |
| 60 per second | ~93 per second | 27.8% of one core | 2.99 ms |
| 30 per second | ~61 per second | 17.5% of one core | 2.87 ms |

CPU per frame barely changes, so the cost is a fixed price per frame rather than something
that amortises: halving the frame rate roughly halves the CPU. egui draws a frame for
each pointer event and one more for the repaint that follows it, which is why a drag at
60 events per second costs about 93 frames per second.

That also means the remaining ~2.2 ms is reachable in principle - about 17-18% of a core
at 60 frames per second - but not from inside this app. eframe 0.31 has no frame-rate cap
(`NativeOptions` exposes vsync and nothing else, and its renderer ignores egui's repaint
delay in favour of its own callback), so capping would mean driving winit's event loop
directly instead of using `eframe::run_native`. If you drag with the editor open, that is
the trade still on the table.

None of that moved the user-visible number: it was 26-30% before these changes and it is
26-30% after. The parts attributable to this code are now measured at well under 0.5 ms
per frame, but they were never the bulk. The remaining ~2.2 ms per frame is inside egui's
per-frame pipeline and the OpenGL present at ~90-110 Hz. Lowering it would mean capping
the editor's frame rate, which eframe does not expose, or a smaller window. Closing the
editor removes it entirely while leaving the overlay running.

To reproduce the idle numbers:

```powershell
<release exe>
powershell -NoProfile -ExecutionPolicy Bypass -File tools/measure-idle.ps1 -Seconds 180
```

Run the editor with `--ui-stats` to watch its frame, preview and save rates while you use
it.

## How this was built

The original Crosshair X installation was inspected first — its Electron archive, its
preset schema, its emitted SVG geometry, its window flags and its default keybinds — so
that this replacement behaves the same way where behaviour is visible to users. No code,
assets, artwork, community content or translations from it were copied; see
[`docs/INSPECTION.md`](docs/INSPECTION.md) for the evidence trail and for what was
deliberately left out.

## Limitations

* Desktop overlays cannot draw over **exclusive fullscreen** (DirectX exclusive) games;
  use windowed or borderless fullscreen, which is what most modern games default to.
  The original app solves this with an Xbox Game Bar extension, which is out of scope here.
* Firing/bloom animations (`firingOptions` in imported presets) are parsed but not
  animated: it needs a global low-level mouse hook, and this build deliberately installs
  no input hooks.
* Sound effects, the Steam Workshop/community browser, accounts and ads from the original
  are intentionally not reproduced.
* HDR displays may shift overlay colours slightly, as with any layered window.
* Overlays of any kind can interact with anti-cheat rules. This app never reads or writes
  game memory and never injects into another process, but use it at your own risk and
  check the rules of the games you play.

## License

MIT. See [LICENSE](LICENSE).
