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
  /// True when this drag may upgrade to Pointer Lock (mouse pointer + API
  /// present). Touch / pen keep the cursor-follows-finger model.
  wantsLock: boolean;
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

  // If the user hits Esc (or the browser drops the lock for any other
  // reason) mid-drag, treat it as a drag cancel: clear our state and let
  // pointerup arrive with nothing to do. Without this, a subsequent
  // pointermove would keep accumulating motion the user did not intend.
  useEffect(() => {
    if (typeof document === "undefined") return;
    const onLockChange = () => {
      const drag = stateRef.current;
      if (!drag || !drag.wantsLock) return;
      if (document.pointerLockElement !== drag.element) {
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
        wantsLock,
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
