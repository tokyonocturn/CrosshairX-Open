# Inspection of the original Crosshair X installation

This document records what was learned from the shipped Crosshair X build before any
code for CrosshairX-Open was written, so that the replacement could match the behaviour
users can actually see. It is an evidence trail, not a specification to copy code from.

The REA MCP server was not registered in the session where this work happened, so the
artifact was inspected directly: the Electron archive is a documented format (an 8-byte
pickle header followed by a JSON index and the file payloads), and the app bundle is
unminified-by-module JavaScript that ships alongside its own resources. Everything below
comes from files on disk, with the location quoted so it can be re-checked.

## Artifact identity

Taken from `C:\Users\kevin\Downloads\CrosshairX\`:

| Item | Value |
| --- | --- |
| Product | Crosshair X by CenterPoint Gaming |
| Version | `15.5.6` (`package.json` inside `resources/app.asar`) |
| Electron | 31.7.7 (`cmake-js.runtimeVersion`) |
| Installation size | ~181 MB `CrosshairX.exe`, 108 MB `resources/app.asar`, 102 MB `app.asar.unpacked` |
| Distribution | Steam (`config/environmentVariables/envVars.json`: `"DISTRIBUTION": "steam"`) |
| Native modules | `cpgwinrt`, `wineventjs`, `wininputjs`, `xinputjs`, `node-hid`, `koffi`, `steamworks.js`, Discord and DualSense SDKs |

The renderer is a React app (`out/renderer/assets/*.js`) and the main process is
`out/main/index.js`. Translations for 20 languages live next to the archive in
`resources/translations/` (2,400 keys in `en_US.json`), which is a compact inventory of
every user-facing feature.

## Feature inventory

From the translation catalogue and the main process:

* **Designer** — centre dot (square/circle/ring/diamond), four arms (rectangle/triangle),
  outline, per-element colour, opacity, thickness, length, gap, rotation, blur.
* **Presets** — saved crosshairs, "Featured"/"Community"/"Top 100" pages, share codes,
  Steam Workshop integration.
* **Position & Size** — display selection, offset from centre, global size, "ignore DPI
  scaling", "limit crosshair size".
* **Display modes** — "Windowed Fullscreen" and "Exclusive Fullscreen" (the latter via an
  Xbox Game Bar extension, with a tutorial page).
* **Toggle/position shortcuts** — `Alt + Shift + Z` to toggle, `Alt + Shift + arrows` to
  move the crosshair one pixel.
* **Firing animations** — bloom, "T shape when firing", per-element firing colours,
  left/right mouse triggers, autoplay loops.
* **Extras** — profile-per-game process detection, streamer server, controller/DS4
  support, launch on startup, minimising to tray/taskbar, custom cursor promotion for
  Mouse X and Scope X.

Not reproduced here: accounts, Steam authentication, Workshop/community browsing, ads,
analytics, the Game Bar extension and the native input/controller modules.

## Preset schema

Community data ships inside the archive (`community-backup/tutorial/crosshairs.json`).
One design is an array of layers, each layer being:

```json
{
  "dot":   { "diameter": 4, "opacity": 0.52, "color": "#FF0000", "shape": "ring", "thickness": 1, "blur": 1 },
  "line":  { "length": 8, "thickness": 2, "offset": 8, "opacity": 1, "color": "Red",
             "shape": "rectangle", "rotation": 0, "blur": 0,
             "visible": ["left", "right", "top", "bottom"] },
  "outline": { "thickness": 4, "opacity": 0, "color": "#000000", "blur": 0 },
  "firingOptions": { "tShapeWhenFiring": false, "firingOffset": 91, "bloomDirection": "inward",
                     "duration": 0.5, "startDelay": 0, "finalOpacity": 0.3,
                     "dot": { "color": null }, "line": { "color": null }, "outline": { "color": null },
                     "mouseButton": "right", "loop": false },
  "position": { "x": 0, "y": 0 },
  "layer": { "size": 1 },
  "migrationVersion": 1
}
```

A saved preset wraps one or more such layers as
`{ "crosshairId": [ ... ], "name": "AK-47", "timestamp": 1607035094735 }`, and a page of
them is `{ "items": [ ... ] }`.

Notes confirmed in the data: colours appear as hex, as CSS names (`"Red"`, `"Black"`) and
as `"r,g,b"` tuples; `outline.thickness` of 0 disables the outline; `visible` selects
which arms draw; `layer.size` scales one layer independently of the global size;
`position` offsets a layer from the screen centre.

CrosshairX-Open reads all of these forms (`crates/crosshair-core/src/preset.rs`) and
ignores fields it does not implement, such as `firingOptions`.

## Rendering geometry

`out/main/index.js` contains the SVG generator the app pushes to its overlay window. The
relevant functions (verbatim names from the bundle) are `getCrispRectangleLinePropsForDirection`,
`getCrispFilledOutlineRectsForDirection`, `getCrispTriangleLinePropsForDirection`,
`getCenterDotProps`, `generateFilledDotOutlineSVG`, `getCrosshairLines` and
`generateCrosshairSVG`. What they establish:

* Layer group: `translate(position.x, position.y) scale(layer.size)`; the arm group and the
  arm-outline group are additionally `rotate(line.rotation)` around the centre. The dot is
  not rotated, except that the diamond is a square rotated by 45°.
* An arm is a rectangle touching the centre gap: for the right arm `x = gap`, `y = -thickness/2`,
  `width = length`, `height = thickness`; the top arm is `y = -(gap + length)`, and so on.
* The rectangle outline is four strips of `outline.thickness` around that rectangle
  (above, below, left, right), not a stroke.
* A triangle arm is a polygon from the gap point out to the two base corners at
  `gap + length`, half a thickness either side; its outline is the same polygon stroked.
* The centre dot is a circle of `diameter/2` (ring: stroked circle with `thickness`),
  a square as `width = height = diameter` centred on the origin, or that square rotated 45°.
  A dot outline is the same shape enlarged by the outline thickness (a ring outline is an
  even-odd ring path).
* Stacking order per layer is: arm outlines, dot outline, dot, arms.
* Blur is an SVG `feGaussianBlur` per element.

CrosshairX-Open reimplements the same rules arithmetically
(`crates/crosshair-core/src/geom.rs`) with unit tests asserting the coordinates, and
rasterises them with `tiny-skia` instead of an SVG DOM. Two deliberate differences where
the original's behaviour is an artefact rather than intent: integer "crispness" rounding
of odd gaps is dropped in favour of exact symmetric geometry, and blur is approximated by
a triple box blur.

## Overlay window

`out/main/index.js` creates the crosshair window as:

```js
{ title: "__CROSSHAIR_OVERLAY", skipTaskbar: true, transparent: true, frame: false,
  alwaysOnTop: true, show: false, minimizable: false, focusable: false, ... }
```

and later calls `setIgnoreMouseEvents(true)` and `setAlwaysOnTop(true, "screen-saver")`.
CrosshairX-Open uses the equivalent Win32 combination (`WS_EX_LAYERED | WS_EX_TRANSPARENT |
WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`) and additionally sizes the window to
the crosshair rather than the screen.

## Settings and keybinds

`dataStoreDefaults` in the main bundle lists the persisted options: `crosshairOffset`
`{x, y}`, `savedPositions`, `crosshairSize`, `limitCrosshairSizeEnabled`, `selectedDisplayMode`,
`crosshairMoveShortcutEnabled`, `crosshairMoveShortcutWasd`, `crosshairSizeShortcutEnabled`,
`rightClickCrosshairToggle`, `autoHideCrosshair`, `streamerServerEnabled`,
`crosshairAnimationsEnabled`, `launchOnStartup`, `minimizeOnLaunch`, `languagePref`, and a
`keybinds` object. Default keybinds:

| Bind | Default | Notes |
| --- | --- | --- |
| `globalShortcut` | `Alt + Shift + Z` | toggle the crosshair |
| position | `Alt + Shift + arrows` | 1px nudges (from the in-app tutorial text) |
| `aim` | right mouse | drives right-button firing animations |
| `fire` | left mouse (hold) | drives left-button firing animations |
| `reload` | `R` | resets paused timelines |
| `randomizer` | `F9` | random crosshair |

`KeybindActions` enumerates the action names: `globalShortcut`, `toggleCrosshair`,
`toggleSavedCrosshair`, `showCrosshair`, `hideCrosshair`, `showSavedCrosshair`,
`loadCrosshair`, `fireAction`, `reloadAction`, `randomizeCrosshair`.

CrosshairX-Open keeps the two shortcuts every user relies on (toggle and position nudge),
makes their modifiers and key configurable, and does not implement the firing/reload/
randomiser bindings because it installs no input hooks.

## What was not taken

No JavaScript, TSX, CSS, SVG, PNG, audio, font, translation or community preset file from
the installation is redistributed here. The preset *schema* is reproduced because file
compatibility is a user-facing behaviour; the numbers quoted above come from the shipped
data files and are used as test fixtures only.
