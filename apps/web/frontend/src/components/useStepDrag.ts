import { useEffect, useRef, type PointerEvent as ReactPointerEvent } from "react";

/// Horizontal pointer-drag helper for "scrub a number by dragging text".
///
/// Used by the status-bar value handles (date, clock, lat, lng, az, alt, fov).
/// Each handle is its own `useStepDrag` instance: the hook is intentionally
/// thin so each call site stays free to clamp / wrap / scale the per-step
/// delta however it wants.
///
/// Pixel-to-step quantisation: values only change when the pointer crosses a
/// step boundary, so single-pixel jitter does not nudge the value.
///
/// On a mouse pointer the hook upgrades to the Pointer Lock API the moment
/// the user crosses the click-slop threshold. While locked the cursor stays
/// parked at its original position and the hook accumulates `movementX`
/// instead of `clientX` deltas, so scrubbing never runs out of room at the
/// screen edge — matching how Figma / Blender / DAW scrubbers behave. Lock
/// is intentionally not requested on pointer-down (a normal click would then
/// trigger Chrome's "Press Esc to exit pointer lock" toast). Touch / pen and
/// browsers without Pointer Lock fall back to the plain clientX path.

const DEFAULT_CLICK_SLOP_PX = 4;

export type StepDragHandlers = {
  onPointerDown: (event: ReactPointerEvent<HTMLElement>) => void;
  onPointerMove: (event: ReactPointerEvent<HTMLElement>) => void;
  onPointerUp: (event: ReactPointerEvent<HTMLElement>) => void;
  onPointerCancel: (event: ReactPointerEvent<HTMLElement>) => void;
};

type DragState<Base> = {
  pointerId: number;
  /// Running total of horizontal motion since drag start. Always derived
  /// from per-event deltas so the same accumulator works for both the
  /// `clientX`-delta path (pre-lock or touch) and the `movementX` path
  /// (post-lock, where `clientX` is frozen).
  totalDx: number;
  /// Last seen `clientX`; used to compute per-event deltas before pointer
  /// lock kicks in. After lock, `event.movementX` is used directly.
  lastClientX: number;
  /// Where the pointer was when the drag started. While Pointer Lock hides
  /// the OS cursor, we paint a small ew-resize indicator at this position
  /// so the user keeps a visual anchor.
  startClientX: number;
  startClientY: number;
  /// True when this drag may upgrade to Pointer Lock (mouse pointer + API
  /// present). Touch / pen keep the cursor-follows-finger model.
  wantsLock: boolean;
  /// Floating cursor indicator element (created lazily once lock is
  /// acquired). Null otherwise.
  indicator: HTMLElement | null;
  /// Currently rendered highlight on the indicator. Tracked so we only
  /// touch the DOM when the sign of `totalDx` actually changes.
  highlight: CursorHighlight;
  base: Base;
  lastStep: number;
  moved: boolean;
  element: HTMLElement;
};

export type StepDrag = {
  handlers: StepDragHandlers;
  /// Returns and clears the "this pointer interaction was a drag, not a
  /// click" flag. Call this from the parent button's onClick to swallow the
  /// trailing click that pointerup synthesises after a drag.
  consumeDragClick: () => boolean;
};

/// `unadjustedMovement: true` opts out of OS-level mouse acceleration so
/// scrubbing maps 1:1 to physical mouse motion. Not in older lib.dom typings.
type LockOptions = { unadjustedMovement?: boolean };

/// Builds the ew-resize cursor SVG with one of three highlight states:
///   - "none"  : both arrowheads bright (hover / no scrub direction yet)
///   - "left"  : only the left arrowhead bright (user is scrubbing left)
///   - "right" : only the right arrowhead bright (user is scrubbing right)
///
/// The three sub-paths (left head / connecting bar / right head) are drawn
/// separately so each can be filled independently. The bar is always bright
/// so the overall shape stays recognisable.
type CursorHighlight = "none" | "left" | "right";

const CURSOR_BRIGHT = "#f5f7fb";
const CURSOR_DIM = "rgba(245, 247, 251, 0.35)";
const CURSOR_STROKE = "#0a0c16";

/// Cursor / indicator dimensions. Sized so the chevrons are unmistakable
/// even at a glance. The actual arrowheads take most of the width — the
/// connecting bar is intentionally a thin sliver so the user reads the
/// shape as "two big triangles" rather than "one little stretched icon".
const CURSOR_WIDTH = 48;
const CURSOR_HEIGHT = 28;
const CURSOR_HOTSPOT_X = 24;
const CURSOR_HOTSPOT_Y = 14;

function cursorSvg(highlight: CursorHighlight): string {
  const leftStroke = highlight === "right" ? CURSOR_DIM : CURSOR_BRIGHT;
  const rightStroke = highlight === "left" ? CURSOR_DIM : CURSOR_BRIGHT;
  // Two simple chevrons (‹  ›). The black "halo" stroke underneath keeps
  // the shape legible against any sky colour without the noise of a full
  // boxed icon. No connecting bar; the gap in the middle is the
  // "this is a horizontal scrubber" hint.
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="${CURSOR_WIDTH}" height="${CURSOR_HEIGHT}" viewBox="0 0 ${CURSOR_WIDTH} ${CURSOR_HEIGHT}">` +
    // Halo: same paths drawn first with a thicker dark stroke.
    `<path d="M 16 5 L 5 14 L 16 23 M 32 5 L 43 14 L 32 23" fill="none" stroke="${CURSOR_STROKE}" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/>` +
    // Left chevron on top of the halo.
    `<path d="M 16 5 L 5 14 L 16 23" fill="none" stroke="${leftStroke}" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/>` +
    // Right chevron on top of the halo.
    `<path d="M 32 5 L 43 14 L 32 23" fill="none" stroke="${rightStroke}" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/>` +
    "</svg>"
  );
}

/// CSS `cursor` value using the neutral SVG (both arrows bright), with the
/// hotspot at the arrow centre. Falls back to the system `ew-resize`
/// cursor if the data-URL form is rejected (very old browsers, CSP rules
/// that block `data:` URLs for cursors, etc.).
export const STEP_DRAG_CURSOR = `url('data:image/svg+xml;utf8,${encodeURIComponent(
  cursorSvg("none"),
)}') ${CURSOR_HOTSPOT_X} ${CURSOR_HOTSPOT_Y}, ew-resize`;

function createCursorIndicator(
  clientX: number,
  clientY: number,
  highlight: CursorHighlight,
): HTMLElement {
  const el = document.createElement("div");
  el.setAttribute("aria-hidden", "true");
  el.style.cssText = [
    "position: fixed",
    `left: ${clientX - CURSOR_HOTSPOT_X}px`,
    `top: ${clientY - CURSOR_HOTSPOT_Y}px`,
    `width: ${CURSOR_WIDTH}px`,
    `height: ${CURSOR_HEIGHT}px`,
    "pointer-events: none",
    "z-index: 2147483647",
    "filter: drop-shadow(0 1px 2px rgba(0, 0, 0, 0.55))",
  ].join("; ");
  el.innerHTML = cursorSvg(highlight);
  document.body.appendChild(el);
  return el;
}

function setIndicatorHighlight(el: HTMLElement, highlight: CursorHighlight) {
  el.innerHTML = cursorSvg(highlight);
}

function removeCursorIndicator(drag: { indicator: HTMLElement | null }) {
  if (drag.indicator) {
    drag.indicator.remove();
    drag.indicator = null;
  }
}

function highlightForDelta(dx: number): CursorHighlight {
  if (dx > 0) return "right";
  if (dx < 0) return "left";
  return "none";
}

export function useStepDrag<Base>(opts: {
  pxPerStep: number;
  clickSlopPx?: number;
  /// Captured once per pointer-down; the captured value is then passed back
  /// into every `onStep` call for this drag.
  onStart: () => Base;
  /// `steps` is the absolute step count from drag origin (can be negative).
  onStep: (base: Base, steps: number) => void;
}): StepDrag {
  const stateRef = useRef<DragState<Base> | null>(null);
  const suppressClickRef = useRef(false);
  const slop = opts.clickSlopPx ?? DEFAULT_CLICK_SLOP_PX;

  // Sync the floating cursor indicator with lock state:
  //   - lock acquired → paint indicator at the original pointer position;
  //   - lock dropped (Esc, tab blur, etc.) mid-drag → remove indicator and
  //     cancel the drag, otherwise a subsequent pointermove would keep
  //     accumulating motion the user did not intend.
  useEffect(() => {
    if (typeof document === "undefined") return;
    const onLockChange = () => {
      const drag = stateRef.current;
      if (!drag || !drag.wantsLock) return;
      if (document.pointerLockElement === drag.element) {
        if (!drag.indicator) {
          drag.highlight = highlightForDelta(drag.totalDx);
          drag.indicator = createCursorIndicator(
            drag.startClientX,
            drag.startClientY,
            drag.highlight,
          );
        }
      } else {
        removeCursorIndicator(drag);
        if (drag.moved) suppressClickRef.current = true;
        stateRef.current = null;
      }
    };
    document.addEventListener("pointerlockchange", onLockChange);
    return () => document.removeEventListener("pointerlockchange", onLockChange);
  }, []);

  const releaseLockIfHeld = (element: HTMLElement) => {
    if (typeof document !== "undefined" && document.pointerLockElement === element) {
      document.exitPointerLock();
    }
  };

  const endDrag = (event: ReactPointerEvent<HTMLElement>) => {
    const drag = stateRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    if (drag.moved) suppressClickRef.current = true;
    if (drag.element.hasPointerCapture(event.pointerId)) {
      drag.element.releasePointerCapture(event.pointerId);
    }
    removeCursorIndicator(drag);
    releaseLockIfHeld(drag.element);
    stateRef.current = null;
  };

  const handlers: StepDragHandlers = {
    onPointerDown: (event) => {
      if (event.button !== 0) return;
      const element = event.currentTarget;
      // Pointer capture is the right primitive for touch / pen (and for the
      // pre-lock phase on mouse): events keep flowing to this element even
      // if the pointer leaves its bounds.
      element.setPointerCapture(event.pointerId);
      const wantsLock =
        event.pointerType === "mouse" &&
        typeof document !== "undefined" &&
        typeof element.requestPointerLock === "function";
      stateRef.current = {
        pointerId: event.pointerId,
        totalDx: 0,
        lastClientX: event.clientX,
        startClientX: event.clientX,
        startClientY: event.clientY,
        wantsLock,
        indicator: null,
        highlight: "none",
        base: opts.onStart(),
        lastStep: 0,
        moved: false,
        element,
      };
    },
    onPointerMove: (event) => {
      const drag = stateRef.current;
      if (!drag || drag.pointerId !== event.pointerId) return;

      const locked = drag.wantsLock && document.pointerLockElement === drag.element;
      if (locked) {
        drag.totalDx += event.movementX;
      } else {
        drag.totalDx += event.clientX - drag.lastClientX;
        drag.lastClientX = event.clientX;
      }

      if (!drag.moved && Math.abs(drag.totalDx) >= slop) {
        drag.moved = true;
        // Defer the lock request until we know the user is actually
        // scrubbing (slop crossed). This keeps a plain click from
        // triggering the browser's "Press Esc to exit pointer lock" toast.
        if (drag.wantsLock && !locked) {
          try {
            const req = drag.element.requestPointerLock({
              unadjustedMovement: true,
            } as LockOptions);
            // Newer spec returns a promise; swallow rejection (some browsers
            // reject `unadjustedMovement` and fall back automatically).
            if (req && typeof (req as Promise<void>).catch === "function") {
              (req as Promise<void>).catch(() => {});
            }
          } catch {
            // Older spec returns void; nothing to do on failure.
          }
        }
      }

      // Keep the floating indicator's highlight in sync with the scrub
      // direction. Only repaint when the sign actually flips, so dragging
      // continuously in one direction does not thrash the DOM.
      if (drag.indicator) {
        const nextHighlight = highlightForDelta(drag.totalDx);
        if (nextHighlight !== drag.highlight) {
          drag.highlight = nextHighlight;
          setIndicatorHighlight(drag.indicator, nextHighlight);
        }
      }

      const steps = Math.trunc(drag.totalDx / opts.pxPerStep);
      if (steps === drag.lastStep) return;
      drag.lastStep = steps;
      opts.onStep(drag.base, steps);
    },
    onPointerUp: endDrag,
    onPointerCancel: endDrag,
  };

  const consumeDragClick = () => {
    if (!suppressClickRef.current) return false;
    suppressClickRef.current = false;
    return true;
  };

  return { handlers, consumeDragClick };
}
