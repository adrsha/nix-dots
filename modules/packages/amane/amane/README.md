# Island

Click the clock to toggle a slim horizontal status row with date/time, MPRIS
animated Mango workspace pills, playback controls, volume and brightness. There is no close
button: click the row background or clock again to collapse it.

Hover the date for a local-month calendar, playback controls for a Now Playing widget,
volume/brightness icons for vertical sliders, and the palette icon for an HSV color
picker. Scroll an icon or slider to adjust; drag a slider to set a level. Right-click
volume to mute/unmute. Popups remain open while moving from the bar into them, with a
300ms dismissal grace. The calendar is refreshed on opening, including after a
date/month change.

The status surface follows the pill's animated dimensions, with an 8px top
margin. Dimensions use an even logical-pixel grid; the spring settles exactly
after 360ms, and text never scales during transitions. Replacement content
fades through a brief blur then becomes crisp. Popup rectangles begin at the
hovered control's bounds inside the island. A shared GPU mask Gaussian-blurs
the union of the island and moving rectangle, then remaps alpha through levels.
No separate connector is drawn. A 480ms damped spring moves and expands the
rectangle, overshoots by about 5%, then settles exactly. Text fades in separately
without being distorted. Slider fills interpolate over 100–140ms.

The row uses 24px end padding and 20px module gaps. Volume and brightness
form one module, with a tighter 4px internal gap. Workspace pills fill their
budget instead of adding hidden side padding. The compact row transport has
single-line elapsed/total times on its left and colored vector playback icons
without background pills or disks. Popups use 20px internal
padding and a 12px gap below the bar; their anchor positions come directly from
the row layout budgets. Calendar height follows the number of weeks, and the
media header, timeline, and transport controls share one content width.

The island and popups now use one `island` surface so their backgrounds can
merge inside the same alpha field. The surface temporarily extends downward;
input is restricted to the row, popup and gap between them. It reserves no
desktop space. Keep compositor animations disabled for `island` to avoid a
second animation on top of Amane's own. No Mango configuration is modified.

Collapsed mode uses a pointer cursor and never opens a calendar on hover.
Hover popups require the expanded row. Mango's `watch all-clients` IPC stream
hides the whole surface whenever a visible real-fullscreen client exists;
fake fullscreen, maximized windows, and fullscreen clients on hidden tags do
not hide it. Popups are dismissed during fullscreen. The stream reconnects on
disconnection and resets to visible while disconnected. This currently treats
any output's visible fullscreen as suppression (the current setup has one output).

Colors are read at startup from `~/.local/state/theme/active.json`; only the
island background is forced to AMOLED black. Global theme files are untouched.
Inter and Material Icons Round must be available through fontconfig (currently
installed in the user's local fonts directory).

### Rendering notes

Content-replacement blur uses Amane's canvas blur, not desktop backdrop blur.
The goo shader separately convolves only the shared background mask; blur and
levels are disabled at rest. The shader file must be deployed alongside
`popups.rs` (the renderer resolves `goo.wgsl` from that source directory).

References: [Android RenderEffect](https://developer.android.com/reference/android/graphics/RenderEffect),
[RuntimeShader/AGSL](https://developer.android.com/develop/ui/views/graphics/agsl/using-agsl),
and [Mango IPC](https://github.com/mangowm/mango/blob/main/docs/ipc.md). Nord gradients are used
for the media fallback tile and vertical slider fills; icon glyphs themselves
remain solid for readability. The media tile is currently a gradient fallback,
not downloaded album artwork. The calendar has previous/next-month navigation; today is highlighted only
in the current month. Event integration is not included.

## IPC commands

Run these while the shell is running, or use them as compositor spawn commands:

```sh
amane ipc call island                 # toggle expanded row
amane ipc call popup calendar         # keyboard/preview access
amane ipc call popup media
amane ipc call popup volume
amane ipc call popup brightness
amane ipc call popup color
amane ipc call popup hide
amane ipc call media play-pause
amane ipc call media previous
amane ipc call media next
amane ipc call volume up              # +5 percentage points
amane ipc call volume down            # -5 percentage points
amane ipc call volume mute
amane ipc call volume 40              # absolute percentage, clamped to 0–100
amane ipc call volume show            # display current volume without changing it
amane ipc call brightness up
amane ipc call brightness down
amane ipc call brightness 40
amane ipc call brightness show
```

Volume changes from other applications also trigger the compact OSD. The open
panel stays open while adjusting controls; compact OSDs appear when collapsed.
Media needs an MPRIS-compatible player on the session D-Bus. Audio uses the
default PulseAudio/PipeWire-Pulse output device.Brightness uses the existing DDC/CI implementation, **not laptop backlight**.
It targets display `1` in `src/ddc.rs`; use `ddcutil detect` to check that choice.
DDC/CI must be enabled in the monitor's menu and your user needs access to its
I²C device. The Nix module enables I²C and installs ddcutil, but permissions may
still need checking. Until the first successful read the UI reports DDC
unavailable. Changes are queued on a worker thread and the displayed value is
the requested value, not a verified hardware readback. External ddcutil changes
are not monitored after startup.

## Apply changes

The Nix module links `~/.config/amane/src` to a Nix store copy. Rebuild/switch
your NixOS configuration to deploy the new sources, then run `amane compile --restart`.
Restarting alone before deployment still uses the old store copy.
No Mango configuration changes are required or made here.

For local iteration, bypass the store copy:

```sh
amane target /home/chilly/flakes/modules/packages/amane/amane
amane compile --restart
```

Return to the managed copy with `amane target ~/.config/amane`.

### Workspace and shader details

The former media-title slot contains tall gradient workspace pills. The current
workspace is wider, taller and brighter; width, height and tint follow
velocity-preserving springs. Clicking a pill focuses that Mango workspace.
The play/pause button uses explicitly centered vector geometry and a subtle
active-theme gradient, not a font glyph with asymmetric bearings.

During popups, the shader owns a single padded rectangle spanning the entire
island, popup and intervening area. Continuous pointer coverage also spans that
gap. Gaussian convolution integrates the vertical spans analytically and uses
40 horizontal quadrature samples rather than a sparse 2D grid. The rest state
still uses exact antialiased rounded rectangles without continuous blur cost.

The expanded row order is volume/brightness → workspaces → clock/date → media.
Modules use their own content budgets without artificial centering wings.
Elapsed/total times stay on one line. Playback uses colored 24px-high vector
play/pause symbols without a background disk or prev/next backplate. Now
Playing has 8px control gaps, 10px section gaps and a 204px outer height.
Calendar header arrows navigate months, including across year boundaries.
Goo shader code is unchanged.

## Modular layout, workspaces, visualizer and notifications

`src/island/modules.rs` is the layout configuration: edit `COLLAPSED` and
`EXPANDED` to reorder or remove modules in either mode. Time and Date are separate
so the shared time glyph moves continuously while the expanded date slides in.
Module widths, row dimensions, popup anchors and insertion motion derive from
these lists. Volume/brightness remain a compact grouped module.

Occupied Mango workspaces use the theme CYAN accent, preserving empty-workspace
geometry/gradients and active-workspace springs. Hover an occupied pill to see a
three-column grid of its windows; click a tile to focus that window. Installed
app icons/names are matched when available, with an initial fallback otherwise.
The grid scrolls beyond three rows. A live `watch all-clients` socket subscription
is decoded by the bundled Python 3 standard-library helper (embedded in the
binary; no pip packages or generated Cargo dependencies). App titles are encoded
safely and never interpolated into shell commands. Client focus uses Mango
`dispatch focusid,client,ID`.

The rightmost visualizer is decorative, following MPRIS playback, not audio
capture. Four rounded bars spring/fade in on playback and out on pause/stop. It
is shared across collapsed/expanded modes, retaining its phase during expansion.

Notifications appear below either mode instead of replacing island contents.
Each card uses the existing goo shader unchanged, emerging from the island or
previous card, with content drawn separately. Up to five cards are visible;
additional arrivals queue. Cards expire after seven seconds of unhovered display;
critical notifications stay until dismissed. Hover pauses expiration, × or
right-click dismisses, and clicking invokes the default notification action.
Replacement IDs update the existing card. Fullscreen hides the whole surface.

## Latest behavior

- Occupied workspace pills use the selected workspace's bright blue accent,
  not cyan; empty-workspace geometry and gradients are unchanged. Window grids
  show icons only (no app names or window-title labels).
- The entire surface hides on occupied focused workspaces, not only fullscreen.
  It returns on an empty workspace. Expanded mode collapses three seconds after
  leaving the row/popups; entering either cancels that deadline.
- Play uses a softer rounded outline; pause is two capsule-shaped pills.
  The visualizer smoothly cycles green/cyan/blue/yellow theme accents.
- Island/workspace/visualizer springs settle over 620ms, goo popups/notifications
  over 720ms, slider fills over 220–260ms, and replacement fades over 340ms.
- Notification gaps are 6px, card padding 24px. App icons render separately from
  attached images. Action buttons invoke their supplied action keys. Progress
  bars reflect the notification `value` hint, when supplied (0–100).
  A Python/busctl session-bus monitor bridges this hint, which the current Amane
  API discards. If bus monitoring is unavailable, other notification content
  still works; arbitrary custom application widgets are not part of the standard
  notification protocol.

## Edge reveal, OSD and popup reflow

Occupied workspaces now slide the island upward offscreen rather than unmapping
it. A 3px top-center edge sensor reveals it; leaving the island/popups tucks it
away after a short grace period. `amane ipc call island reveal` reveals without
changing expanded/collapsed mode; the existing no-argument call reveals and
toggles. Real fullscreen still hides the layer entirely.

Volume symbols distinguish muted, zero, low (1–49%) and high (50%+) levels in
both the row and volume popup. Expanded OSD events reveal the normal volume or
brightness slider popup for two seconds instead of replacing the row.

Popup open/close duration is 420ms. While separating/returning, popup corner
radius grows toward a capsule; the shared goo shader remains unchanged. Fast
hover switches retarget the existing box's x/width/height springs without
resetting its open amount or origin, producing a moving-tooltip transition.
Notification stack displacement follows opening/closing card occupancy and the
current hover-popup extent, with spring interpolation rather than jumps.

## Screen frame and input alignment

The single `island` surface now fills only the output the compositor assigned to
it, drawing a 4px black screen frame with an otherwise transparent/click-through
interior. The top-center edge sensor remains interactive. Keeping surface bounds
fixed removes pointer-coordinate shifts caused by resizing/recentering the layer
while expanding. Module slots remain in a stable order even while invisible;
zero-scale hidden modules cannot receive input. The clock has its own explicit
click target. This avoids target-index reuse during an in-flight click/hover.

The hiding island shares the unchanged goo shader with the frame's top edge;
only the narrow local merge strip runs the blur/threshold effect. Popup and
notification corners remain at their readable-content radius throughout the
content fade, and approach a capsule only once content opacity reaches zero.

## Slower hiding and notification layout polish

Edge activation spans the middle half of the screen. Exposure spring time is
scaled by 2.2 (about 1.36s settle), independently of module/popup motion. Expanded
mode is ineligible for hiding; after collapse it remains ineligible for the full
620ms row transition before the hide slide may begin.

Notification plain/action/progress/image samples were triggered individually and
screenshots inspected. Text containers now use `Text::height_in` at their actual
width instead of reserving maximum-line heights; card height derives from the
rendered content plus 24px end padding. Action labels are centered in measured
containers and progress labels have a consistent right-aligned slot.

## Settings and lower-edge islands

The rightmost gear in the expanded main island opens vertical settings. An icon-only sidebar selects a single section; its menu toggle expands to
icons plus labels. Section content scrolls independently, with page fades and
hover labels on collapsed navigation. History retains the
latest 200 notifications across restarts in `$XDG_STATE_HOME/amane/notifications.v1`
(default `~/.local/state/amane`), with private directory/file permissions (700/600).
History uses the live card renderer (same icon/image, text, progress and action
layout), with remove and clear controls. Action labels remain visible in archives;
buttons are disabled once the original notification is no longer live. D-Bus IDs
are never serialized, so archived actions cannot dispatch to a different app after
restart. Older text-only history records migrate without inventing rich metadata.
Bounded image copies are retained privately in `amane/notification-images`; unused
generated cache files are pruned when the archive changes.

The network section uses NetworkManager when available. On this machine networkd
is retained: live interface state/IP addresses are shown and unavailable Wi-Fi
controls are explained, rather than showing a false offline state. No network
configuration is changed. Bluetooth uses BlueZ for power, discovery and device
connect/disconnect/pair; PIN/passkey pairing needs an external pairing agent.

Customization keeps the 4px black frame and persists its inner corner radius
(default 24px, adjustable 0–96px). Theme cards read swatches from
`~/.config/theme-manager/themes` and launch `theme-set NAME` with a literal argv.
Active Amane colors reload from `~/.local/state/theme/active.json` automatically.

IPC entry points for keybindings:

```sh
amane ipc call settings                 # toggle vertical settings
amane ipc call settings history         # also network/bluetooth/customization
amane ipc call settings hide
amane ipc call auxiliary screenshot     # bottom-right: window / region / full
amane ipc call auxiliary record         # top-left side: region / full, owned stop
amane ipc call auxiliary session        # lower-left side: confirmed session actions
amane ipc call auxiliary search         # bottom-center: type, arrows, Enter, Esc
amane ipc call auxiliary search firefox # optional initial query
amane ipc call auxiliary hide
amane ipc call picker                    # toggle screen color picker (show/hide/status)
```

## Screen color picker

`amane ipc call picker` freezes the screen once with `grim` (P6 netpbm, sampled
locally so only one capture runs), tucks the island and auxiliary menus away,
and shows a full-screen overlay: a crosshair, a 15×15 magnifier centred on the
pointer, and a readout card with a swatch, `#rrggbb` hex and RGB values. Left
click copies the pixel under the pointer as hex with `wl-copy` and exits, writing
`$XDG_STATE_HOME/amane/picked-color` and running `~/.config/amane/hooks/color HEX`
when present. Right click hands the pixel to the colour popup, which opens from
the bar preset to that colour with clickable rows for `#rrggbb`, `rgb(r,g,b)` and
`hsl(H, S%, L%)`. Escape cancels, as does `picker hide`; `picker copy` is the
right-click action exposed for keybindings. `picker status` reports
`active`/`inactive`. Requires `grim` and `wl-copy` on `PATH`.

Auxiliary action menus are vertical. Screenshot emerges from the lower-right
lateral border, sessions from the lower-left lateral border, and recording from
the upper-left lateral border. Search rises from the bottom-center border as a
standalone pill; typing opens a separate goo-connected results container above
it. The best match is nearest the pill, with subsequent results stacked upward.
Menu contents stay unscaled and input is disabled until they become readable.

All auxiliary islands can also be revealed by touching their 4px edge sensors:
- Recording: left edge, 6–32% of screen height.
- Sessions: left edge, 55–91% of screen height.
- Screenshot: right edge, 55–91% of screen height.
- Search: middle half of the bottom edge.

Leaving a menu schedules a three-second tuck-away; hovering it pauses that.
Edge reveal uses the same analytic spring family as the main island, with the
existing goo shader unchanged. Busy capture workers disable reveal sensors.
Theme surfaces use ONE_BG, ONE_BG2 and BASE00; literal grays are gone. Shared
click feedback gives buttons a small damped scale-down/rebound, including main
media/controls, workspace tiles, notification actions and settings navigation.
Search accepts `s QUERY` for browser search and `s https://example.org` for URLs.
Installed desktop commands are launched as argv, never by interpolating the query
into a shell. Search takes keyboard focus only while open and releases on hide.

Screenshots are saved under `~/Pictures/screenshots` and copied as PNG to the
clipboard, then offer Save or Edit. Edit opens Satty and copies the annotated PNG;
annotated files use an `-edited.png` suffix. Recording output goes under
`~/Videos/recordings`. The shell owns a pipe to the recorder worker; Stop and shell
exit close only that worker's recorder, never unrelated recording processes.
Session actions require a confirmation; lock uses swaylock with current theme
colors and Nix-configured PAM authentication.

`../amane.nix` now declares grim, slurp, wf-recorder, wl-clipboard, Satty, swaylock,
xdg-utils and Python. These tools require a normal NixOS rebuild before use;
Amane displays missing-tool errors. No rebuild or real capture/recording/session
operation was run while developing the UI. Capture/browser/desktop-launch argv
contracts have isolated mocked tests (`python3 src/auxiliary/test_actions.py`).

### Complete IPC reference

```sh
amane ipc call island                       # toggle main row
amane ipc call island reveal                # or show: reveal without toggling
amane ipc call volume up                     # down | mute | show | 0–100
amane ipc call brightness up                 # down | show | 0–100
amane ipc call media play-pause              # previous | next
amane ipc call popup calendar                # media | volume | brightness | color | hide
amane ipc call popup workspace ID
amane ipc call settings                      # history | network | bluetooth | customization | hide
amane ipc call auxiliary screenshot          # record | session (or sessions) | search [query]
amane ipc call auxiliary hide
amane ipc call auxiliary status
```

### Accent-led surfaces and lateral motion

Settings and auxiliary action cards now emphasize colored icons and 2–3px accent
outlines on transparent/black surfaces, rather than theme-gray tiles. Theme cards
use their own preview accent as their outline. Existing button click springs remain.
Side islands translate at their complete rendered size, rightward from the left
border or leftward from the right border. Content follows that exact translation
with a synchronized fade; extra rounding begins only below the zero-content-alpha
threshold. Neither body width nor body height is multiplied by reveal progress.
Search keeps its separate upward-results behavior.

### Compact side controls

Screenshot/recording/session strips are now 46px wide, with 38px icon hit areas,
no headers, inline labels, individual button backgrounds or button outlines.
Hover reveals an external tooltip; status/error text is available from the info
icon or `auxiliary status`. Session confirmation still requires the confirm icon.
The border-side goo source includes the solid offscreen half-plane: blurring a
4px-only strip previously erased the source before it could form a neck. The goo
shader itself is unchanged. Extra rounding still waits for content to fade.

Decorative outlines were removed from settings controls, theme cards, history
cards, notification actions and search results. Remaining essential outlines are
4px (selected navigation and the curvature preview); archive cards use thin
separator lines rather than outlines.

### Snappy feedback and popup-like tooltips

Button feedback is now one 160ms press/release (10% scale change, 40ms press,
120ms ease-out), with no oscillating subpixel tail. Expanded time and date both
hover-open the calendar; collapsed time retains its click-only behavior.

Switching auxiliary menus queues the next menu while the current one slides all
the way into its own border, then reveals the queued menu. Recording/start/stop
icons use theme-manager's RED. Auxiliary tooltips now use the same spring/goo
approach as the main popups: anchored at the hovered icon, spring-retargeted
position and width, unscaled fading text, 300ms leave grace, and pointer input
bridging to keep them open while crossing the gap. Extra rounding waits until
text has faded, and menu dismissal also closes its tooltip.

### Auxiliary toggles and modal backdrop

`auxiliary screenshot`, `record`, `session`/`sessions`, and `search [query]` now
**toggle** their respective menu. Repeating a command closes that menu (or cancels
its queued reveal); selecting a different menu keeps the animated handoff.
`hide` always closes, `status` remains read-only, and edge-hover remains reveal-only.

Search and its results now translate vertically at full width/height rather than
scaling. The placeholder is just “Search applications…”. The `s ` web prefix still
works, but is not advertised inside the field. A solid offscreen bottom source
keeps the goo connection intact; rounding is delayed until content has faded.

Only auxiliary menus have a full-output dim/pointer shield. Outside clicks and
scrolls are swallowed, not forwarded or used to dismiss; Escape or the IPC toggle
closes the menu. This is a layer-surface input shield, not a compositor security
grab: compositor-reserved global bindings can still be handled by the compositor.
The main island is unchanged. Capture workers disable the backdrop and shield
immediately before tool selection/capture, and recording keeps them disabled until
its owned recorder exits. Actual capture and compositor-global shortcut behavior
have not been end-to-end exercised.

### Main IPC cycle and search editing

`amane ipc call island` now cycles hidden → collapsed → expanded → hidden.
Explicit `show`/`reveal` still only reveal. Forced hiding works on empty workspaces
as well; the edge sensor can reveal it again.

Search now has its own UTF-8-safe caret editing (Left/Right/Home/End/Backspace),
a spring-positioned blinking caret, and a brief insertion fade/offset. Ctrl+J/K
navigate results like Down/Up. A minimal keyboard transport patch preserves those
two control characters; the custom search handler implements their behavior.
The patch is stored in `src/patches/ctrl-search-keys.patch` and applied by the Nix
Amane package override. The running binary was built with it. Until the Nix rebuild,
the old `amane compile` CLI overwrites its cached library with the unpatched copy;
use the patched package after rebuilding to retain these keys on future compiles.

Search closure is sequenced: results merge for 460ms into the pill's exact bounds
and corner radius, then the pill slides down. Selecting a result defers application
launch until the merge/hide finishes. Results use one continuous, unclipped rounded
background field instead of scissors that previously created square lower corners.

### Optional edge activation and zero-width paused visualizer

The visualizer's animated width and its adjacent gap collapse to zero when
playback stops/pauses. Both collapsed and expanded module positions follow the
released space, rather than leaving a reserved visualizer slot.

Customization contains independent persistent edge-activation switches for main,
screenshot, recording, session and search islands. IPC activation is unaffected.
The preferences default to enabled and are stored in
`$XDG_STATE_HOME/amane/edge-activation` (default `~/.local/state/amane`).

```sh
amane ipc call edge status
amane ipc call edge main off
amane ipc call edge screenshot on
amane ipc call edge record toggle
amane ipc call edge session off
amane ipc call edge search on
```

Each island accepts `on`, `off`, `toggle`, or `status`; omitting the action toggles.
`island`, `recording`, and `sessions` are aliases for `main`, `record`, `session`.

### Settings scroll and edge switches

Edge activation uses icon-led switch rows showing each island's screen-edge
location, with colored on/off tracks and thin separators. IPC access is unchanged.
Settings pages and nested network/device lists use settings-only scroll physics:
56px per input line (previously 15px), smoothly animated motion, bounded 54px
rubber-band overscroll, and automatic spring-back after scrolling stops. A slim
accent scrollbar appears when content overflows; its full-height track supports
click-to-position and dragging. Each settings page keeps its own scroll position.
The content viewport clips both rendering and pointer targets, keeping offscreen
controls inactive; controls unrelated to settings retain their old scroll speed.

### Compact history and connectivity pages

Notification archive cards are capped at the same 400px width as live toasts,
using the same rich renderer. Network settings now separate a connection-status
hero, live wired/wireless interface rows with colored icons and IP information,
and an explicit unavailable-Wi-Fi-backend state. NetworkManager-capable systems
also show nearby secured/open networks with signal values and join controls.
Networkd remains unchanged. Bluetooth has a status hero, power/discovery controls,
paired/nearby groups, device-type icons, connection/battery state and empty states.
Existing BlueZ connect/disconnect/pair behavior is retained; PIN pairing may need
an external agent. No live connections were changed during UI verification.

Search editing additions: Ctrl+Backspace deletes the previous whitespace-delimited
word, Ctrl+Delete deletes the next word. Ctrl+J moves visually upward through the
bottom-to-top results; Ctrl+K moves downward. Empty or already-merged results skip
the 460ms merge wait when hiding the pill. The keyboard transport patch now tracks
Ctrl modifiers to distinguish Ctrl+Delete/Backspace from ordinary editing.

### Main visibility permissions

Settings → Customization includes two independent persistent switches:
- Auto-hide main island (default on): off suppresses automatic occupied-workspace
  tucking; explicit IPC hiding/revealing still works.
- Allow over fullscreen (default off): removes fullscreen suppression for the main
  island surface and its associated popups/notifications. This does not disable
  auto-hide: turn auto-hide off as well for continuous visibility over fullscreen.

Preferences persist in `$XDG_STATE_HOME/amane/main-visibility`. Expanded-row
collapse behavior, auxiliary islands and per-island edge activation are unchanged.
Fullscreen display still depends on the compositor honoring overlay-layer ordering;
real fullscreen interaction was not end-to-end tested.

### Theme-card hit testing and workspace hover removal

Fixed a library hit-test clipping error that made translated/scrolled controls
visible but unclickable. Clipping now maps the viewport into each target's local
coordinate space before intersecting; regression tests cover both visible and
hidden scrolled targets. The patch is shipped as `src/patches/scrolled-hitboxes.patch`
in the Amane Nix override, alongside the keyboard patch. Both are active in the
running binary; until rebuilding the patched CLI, the old `amane compile` may
restore the unpatched cached library.

Theme cards now show the applied theme from `active.json` with an Active checkmark,
serialize theme requests and verify that theme-daemon applied the requested name
before reporting success. `theme-set` remains the sole theme-changing mechanism.
Workspace pills no longer open app-grid hover popups. Click-to-switch and occupied
colors are retained; `popup workspace` returns a removal notice instead of opening.

### Theme backgrounds, AMOLED and outside dismissal

Island bodies, side/search islands, popup goo, screen-edge frame and notification
cards use the active theme's BASE00 background rather than hardcoded black. Goo
alpha/geometry is unchanged; its RGB comes from the live theme. Choose “AMOLED
black” alongside the theme cards to use true-black backgrounds. This uses the
existing theme-manager `theme-set --amoled on|off` global persistent preference,
not a separate per-shell setting; changing themes retains that preference.
The manager's existing selection is respected; fresh defaults are non-AMOLED.

Outside clicks now dismiss auxiliary menus through their existing shield and
merge/hide animation. Expanded main/settings also intercept outside clicks to
close popups/settings and hide the main island. These clicks are swallowed, not
forwarded to underlying apps. The normal collapsed main island remains nonmodal.
Capture workers still remove the auxiliary shield/dim before capture and during
recording. This is output-local layer-surface input, not a global compositor grab.

### Independent AMOLED controls and unified settings

Supersedes the shared AMOLED behavior above. Appearance now has **Amane AMOLED**
(shell-only, saved atomically in `$XDG_STATE_HOME/amane/amoled`, default off) and
**System AMOLED** (theme-manager's existing global switch). The local switch never
runs theme-set or alters global theme state. Static themes recover their original
BASE00 from their theme file when global AMOLED has replaced active.json BASE00.
Dynamic themes reuse a cached non-AMOLED background if available; if Amane has
never observed the unmodified dynamic palette, it cannot reconstruct that color
and retains the published background until a non-AMOLED palette becomes available.

Settings share compact tinted row surfaces, consistent switches/icon sizes,
filled navigation selection, restrained status panels instead of gradient banners,
real filled action buttons and selected-theme tiles. Text contrast is adjusted for
readability even for themes with dim WHITE/LIGHT_GREY aliases. Notification history
retains its shared rich-card rendering and width. No external design references
were fetched (no browsing capability in this session).

### Shared island animation performance

The first timing probe found ~48–50ms in main-island view creation and ~71–74ms
in CPU draw/layout/target preparation (not GPU execution). Closed settings still
constructed and laid out their entire hidden page every frame. That has been
removed after its close animation settles. The duplicated module construction in
normal idle/panel rendering is also removed. The shell itself now uses opt-level 3
in dev builds, in addition to its dependencies. `optimized-shell.patch` preserves
that setting in the Nix-packaged CLI's generated manifest; until rebuilding the
patched CLI, old `amane compile` can regenerate the previous build settings.

Shared goo keeps its original 40-sample edge/neck integration, but uses normalized
constant weights and skips integration deep inside/outside the shapes. Animation
springs, duration, output-sized input surfaces and outside-dismiss behavior remain.
The auxiliary dim backdrop uses a single RGBA fill instead of an opacity group,
avoiding an extra output-sized offscreen/composite pass.

AMANE_FRAMES probes at 1920x1080 on the RTX 4070 SUPER showed warmed main CPU view
near 0–0.1ms and draw preparation near 0.1–0.3ms; main and auxiliary median callback
intervals in the later run were about 5.6ms. These are application-side frame gaps,
not independently measured displayed FPS. The initial baseline was short and hit
a Wayland configure-serial error, so it is not a clean sustained before/after FPS
benchmark. Later probes exercised main toggles, screenshot/session/search toggles,
and settings open/close without that error. Diagnostic logging is disabled again.

### Color picker

The Controls module now has a third icon (palette, CYAN accent) after volume and
brightness. Hovering or clicking it opens an HSV color-picker popup with the same
goo background, spring geometry and 300ms dismissal grace as the other hover
popups. The saturation/value plane and the hue strip both support click and drag;
the swatch, hex label and knob update live. Clicking the hex row copies the value
with `wl-copy`, writes `$XDG_STATE_HOME/amane/picked-color` (atomic, hex + newline),
and shows a transient "Copied" badge for 1.5s. If `~/.config/amane/hooks/color`
exists and is executable, it is spawned with the hex string as its only argument,
letting user scripts emit the color elsewhere (for example `notify-send` or a
compositor IPC). No shell interpolation: the hex is passed as a literal argv and
the hook path is single-quote-escaped. The picker seeds from the theme's BLUE.

```sh
amane ipc call popup color            # open the picker on demand
```

