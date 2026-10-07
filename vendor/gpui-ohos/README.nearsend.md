# NearSend OHOS platform patch

Mirrors `ohos-rs/gpui-ohos` branch `feat/pr82-ability-adaptation`, revision
`d9b2f1a6c1c963bc6edc931da53acb425f22f604`.

`src/ohos/window.rs` uses the demand-driven GPUI VSync scheduler as the
single source of rendering ticks. XComponent's continuous `WindowRedraw`
callback remains a fallback when native VSync creation or requests fail.
This avoids drawing twice per display refresh and advancing touch momentum
from two independent frame sources.

Touch scrolling is adapted by `src/ohos/touch_scroll.rs`, using only GPUI's
public `PlatformInput` interface. GPUI itself is the unmodified git dependency
at revision `f45c7c22d0c04fb12527e71bbbee1dc6bdbdee0a`; no local GPUI source
patch or extra `TouchEvent` field is required.

The framework enables `TouchInputDelivery::Both`. XComponent's system ArkUI
Pan supplies recognition, physical-pixel deltas and release velocity in px/s.
The window converts displacement and velocity to GPUI logical pixels once.
There is no raw-touch pan threshold, velocity history or least-squares fit.

The adapter offers raw `Started` contacts to GPUI so controls can claim a
direct drag. Claimed drags keep their complete raw stream. Unclaimed contacts
are immediately cancelled in GPUI. Compatibility taps use
the ordinary mouse compatibility events, long presses use public phased
`LongPressEvent`. Only native Pan events emit scrolling `ScrollWheelEvent`.
Pan Start applies the system-provided displacement immediately, so the first
accepted packet invalidates the view without waiting for Update. Later packets
are incremental and never replay that initial displacement. A caught fling stops on raw contact Down;
the next native pan chooses its own axis and cannot become a tap. Raw Up and
Pan End may arrive in either order, and neither launches a second fling.
The existing native recognizer uses 8 physical pixels, matching the SDK's
native gesture units. The OHOS ArkUI friction curve is retained.
After compatibility taps, an idle raw cancellation restores GPUI's touch
input modality without emitting another gesture.

Native Pan does not emit post-release displacement. Each window therefore owns
at most one momentum curve, seeded from native Pan End velocity and advanced
once per native VSync. Its origin is release dispatch time, so queued delivery
cannot skip the first inertia frame. Swipe does not launch another curve.
No GPUI momentum frame callbacks are scheduled. A cancellable, one-shot timer
is used only for pending long presses. Host regressions are run with:

```sh
cargo test --manifest-path scripts/ohos-touch-tests/Cargo.toml --locked
```

Window visibility and surface lifecycle are processed before pending frames.
Hidden or destroyed surfaces do not present frames; backgrounding clears
pending frame demand, active contacts, long-press timers and momentum. A
visible surface requests a fresh VSync on resume.
This prevents a queued animation frame from presenting to a surface the
system has just hidden or destroyed.

The runtime sources match the published branch. Keep this snapshot and its
dependency branch in sync when updating the platform integration.

## Framework performance optimization

The adapter reuses background workers and native UI wakes, switches healthy
VSync to Ability `FrameInputDelivery::OnDemand`, and caches compatible GPU
pipelines/layouts/shaders per context. A stalled background queue triggers a
kernel worker-state check; blocked workers permit additional capacity, while
CPU-running workers count toward existing capacity. The 2–8 base workers are
not a concurrency limit. Remaining foreground work is reposted after each
UI-turn budget. Input callbacks, animation frame
rate, rendering quality, window count and capture formats retain their existing
behavior. Continuous XComponent frame delivery remains the native VSync fallback.

The 37 host checks compile the production modules with host platform shims.
Phone and 2in1 framework, fallback and application checks are recorded in
`docs/verification/2026-10-07-system-frame-followup.md` at the NearSend repository root.
The complete implementation status, deferred proposals and physical-device
delivery are recorded in `docs/verification/2026-10-07-optimization-status.md`.

## 2026-10-07 audit fixes

The adapter and Ability now handle failed frame registration, restarted native
Pan sessions, changed effective viewports, and CPU-bound worker saturation.
See `docs/verification/2026-10-07-adapter-audit-fixes.md` for implementation,
FFRT evaluation, simulator results, and explicitly recorded validation limits.
