# Animation foundation

`src/motion.rs` supplies the shared policy, short duration tokens, retargetable
scalar `Tween`, and bounded click `Ripple`. These are state components for Iced
custom widgets. `src/button_feedback.rs` wraps existing Iced buttons with visible
ripples and finite refresh feedback, retaining the original input behavior.
`src/reveal.rs` supplies lazy horizontal/vertical reveals and shared popup state.
`src/motion_strip.rs` supplies keyed insertion, removal, and movement feedback.
No new dependencies, timer subscription, or animation worker thread is needed.

The app detects Windows PE once through the HKLM
`SYSTEM\CurrentControlSet\Control\MiniNT` marker. PE always disables motion.
On normal Windows, `SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION)` controls
decorative animation. Failed queries disable it conservatively. The policy is
read at startup and refreshed on `WM_SETTINGCHANGE` and window focus. The native
window subclass is installed independently of DWM availability.
The client-area flag follows Microsoft's
[SystemParametersInfoW documentation](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-systemparametersinfow).

## Using the components

- Wrap an ordinary button with `button_feedback::wrap(button, app.motion,
  RefreshIcon::None, enabled)` before adding its tooltip. The child button still
  publishes the action on release in the same event. No command timer is used.
- Use `RefreshIcon::Center` with a 14-DIP placeholder for icon-only refresh
  buttons, or `RefreshIcon::Leading` with a placeholder before the label.
  Refresh icons make one 220-ms turn; repeated clicks retarget the same turn.
  This feedback does not represent scan progress and never waits for a scan.
- The wrapper stores one ripple and one spin tween in the widget tree, clips
  drawing to the button and viewport, preserves layout/overlays/operations, and
  cancels both effects when the policy disables motion. Disabled controls retain
  their disabled icon appearance and never start new effects.
- `ui_icons::refresh_frame` shares 32 pre-rotated SVG handles. Rotation is baked
  into vector geometry because the current CPU SVG pipeline does not fully apply
  image rotation. The handle set is bounded; redraws create no SVG source data.
- Keep a `Tween` or `Ripple` in the custom widget's `Tree` state so rebuilding the
  view does not restart it. Pass the current `app.motion` policy to the widget.
- Retarget a tween when the desired visual state changes. Reversals start at the
  current interpolated value. Sample using the redraw event timestamp.
- Call `settle_if_disabled` when synchronizing tween state with policy changes;
  disabled motion immediately returns the target and must not resume later.
- Call `request_frame` while processing widget events. Only active, enabled
  transitions request another redraw; completed or disabled effects remain idle.
- Use QUICK (100 ms) for controls, STANDARD (150 ms) for reveals/movement, and
  FEEDBACK (220 ms) for click feedback. Avoid animating entire file listings.
- Apply actions immediately. Removed content can have a noninteractive visual
  remnant, but must not retain focus, accept clicks, or keep operations alive.
- Clip ripples to their button bounds. Keep one ripple per control and replace
  it on repeated clicks. Use button activation for commands, never the ripple.
- Keep stable tab/panel identities across moves; never key motion by mutable
  array indices. Resizing and pointer dragging must track the pointer directly.

## Reveals

- `reveal::horizontal(open, policy, build)` clips an overlay from its left edge
  without changing workspace dimensions. It drives the compact File tree drawer.
- `reveal::vertical(open, policy, build)` clips a full child layout and varies its
  outer height. It drives the Transfers body and resize grip; the header stays
  interactive. The saved list height and sidebar split are never animated or
  overwritten.
- `reveal::State` gives the anchored Settings popup the same timing, interruption,
  and policy behavior. It reveals from the top when below its anchor, or the
  bottom when positioned above. Placement and outside-click/Escape dismissal
  retain their existing behavior.
- Opening takes 150 ms and closing 100 ms. Logical flags change immediately;
  closing content receives no input, focus operations, or nested overlays.
  Outgoing gestures are cancelled without publishing commands. Redraw events
  still initialize paint styles, with their messages discarded.
- Repeated toggles reverse from the current progress. Disabling motion snaps to
  the final state. Cold-mounted open content starts fully visible, so restored
  layouts do not animate at startup.
- Builders run only while open or finishing a close. Finished hidden content is
  released, and no animation redraws are requested after completion. The File
  tree shade disappears immediately on dismissal so the workspace accepts input.
- `responsive_cache::responsive` reuses built elements while available size stays
  unchanged within the current app view. The app root and Transfers use it to
  avoid reconstructing hidden/visible jobs on animation relayouts. Resize and
  every new app view still rebuild from current data. Transfers height changes
  require parent layout, but do not send per-frame app messages or rescan files.

## Tab motion

- `motion_strip::view(items, policy, paint)` accepts stable keys, ordinary child
  elements, and optional small owned paint snapshots. It reuses widget trees by
  identity when items are inserted, removed, or reordered. The trailing New tab
  control has a separate key and moves with the strip.
- New tabs reveal over 150 ms with a four-DIP rise; surviving tabs move to their
  new positions over 150 ms. Removed tabs leave a clipped, paint-only remnant for
  100 ms. At most 16 remnants are retained under rapid key repeat.
- Selection, insertion, removal, and scan cancellation still happen in the
  original app message handlers immediately. Closing retains only label/selected
  paint data, never a `Tab`, file listing, cancellation handle, or scan task.
- Live button input, drag-marker operations, hover, and tooltip anchors use the
  same last-painted bounds. Overlapping controls are visited in paint order for
  hit testing. Direct dragging continues to track the pointer without easing.
- Moving between existing panels gives source-exit and destination-entry
  feedback. When the last tab removes its source panel, the panel uses the shared
  panel transition described below. Cold-mounted/restored
  strips start settled. Existing active-tab reveal and manual scrolling stay in
  `tab_reveal.rs`; animation frames do not scroll the strip again.
- Positions retarget continuously during repeated edits. Resizing and disabled
  motion snap to final positions. Animation redraws update local paint/input
  geometry, without app messages, layout invalidation, or rebuilding children.
  Exit paint is released when complete, and settled strips request no frames.

## Panel transitions

- `motion_strip::grid` extends keyed motion to two-dimensional rectangles.
  Stable panel widget keys survive tab selection, docking, and index compaction.
  The same host is used for one through four panels, retaining surviving widget
  trees and tab state. Cold/restored views start settled.
- Creation reveals over 150 ms; docking and surviving panels move over 150 ms.
  Logical sizes change immediately. Closing immediately drops tabs and cancels
  their scans, leaving only a path-labelled paint card for 100 ms. This card has
  no input, focus, overlays, directory entries, or task handles.
- Layout topology identifies docking changes independently of split ratios.
  Window and splitter resizing snap to the pointer. An empty Iced pane grid
  underneath the content retains the existing resize behavior. Accepted panel
  clicks cannot start a resize through the animated panel.
- Paint, input, tooltips and docking markers share the last-painted geometry.
  Animation frames translate cached layouts without reconstructing directory
  views or posting app messages. The existing motion policy disables all motion
  in Windows PE and when Windows animation effects are disabled.

## Explicit action highlights

Button feedback is integrated into shared compact/icon/navigation actions, both
ribbon layouts, tab title/close/add buttons, panel controls, and the Settings
button. Refresh spins cover panel navigation, File tree refresh, and the
shadow-copy picker's Refresh button. File-list rows keep their existing feedback.
The File tree drawer, Settings popup, and Transfers dropdown now share reveals.
Tab insertion/removal/reordering now uses the shared keyed motion strip.

The reusable `highlight::wrap` adds a clipped accent wash to rows and an accent
outline to panel/tab drop destinations. It forwards input, focus, geometry and
overlays unchanged. Accepted file drops flash the destination immediately.
Successful New folder/Rename edits and completed Copy/Move roots request row
highlights using actual result paths, including Save as - Copy names. Failed,
skipped, deleted and recursive child items do not produce result highlights.

Only an explicit action can request a highlight. Listing updates resolve those
requests when the result first appears; ordinary listings and size batches never
create or restart one. Requests expire after ten seconds and the registry is
bounded to 64 paths. Highlights fade for 400 ms using absolute timestamps, so
virtualized/offscreen rows cannot replay them when remounted. Visible active
highlights request redraws locally without layout invalidation or app messages;
settled highlights request no frames. Selection and scroll positions are unchanged.
Windows PE and disabled Windows animation effects suppress all highlights.

## Validation

Unit tests cover PE precedence, disabled/zero-duration snapping, interruption,
frame scheduling, ripple lifetime, and disabling during a transition. Widget
regressions check immediate/once-only command dispatch, state across rebuilds,
disabled and captured input, release outside the button, repeated clicks, clipped
ripple pixels, actual refresh rotation on the CPU renderer, and idle frame
scheduling. Reveal regressions cover reversal, lazy builders, no reconstruction
on animation frames, discarded outgoing gestures, exclusion from focus/overlays,
partial pixel/input clipping, popup dismissal during opening, anchor direction,
policy snapping, and idle scheduling. Responsive-cache tests check reuse plus
fresh data after resize/new app views. Keyed-strip tests check pressed-widget
identity across reorder, intermediate paint/hit bounds, immediately actionable
entrances, noninteractive exits, bounded/reused paint snapshots, resize/policy
snapping, continuous retargeting, viewport clipping, idle scheduling, and model/
scan preservation during cross-panel moves. A drag regression covers a moving
tab behind the New tab control. Run the normal locked tests and release
build as well. Native smoke checks still need
normal Windows with Animation effects toggled while the app is open, and a
Windows PE boot. Profile the CPU renderer
with four panels and large directories; verify no idle redraw loop and immediate
input response during repeated toggles, close/reopen, drag/drop, and resizing.

## Shared library boundary

The production primitives, feedback, reveal, keyed-motion, highlight paint, and
responsive-cache implementations now live in sk-iced/src/animation. Local module
names are compatibility re-exports, so existing widget identities and app behavior
remain intact. File Beholder keeps its path-based highlight registry, operation
triggers, tab/panel models and native event hooks. Refresh SVG frames are optional
in the library; File Beholder enables them. Shared animation tests moved with the implementations;
application scan/cancellation/drop regressions remain in File Beholder.
