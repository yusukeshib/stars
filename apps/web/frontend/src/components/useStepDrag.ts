import { useRef, type PointerEvent as ReactPointerEvent } from "react";

/// Horizontal pointer-drag helper for "scrub a number by dragging text".
///
/// Used by the status-bar value handles (date, clock, lat, lng, az, alt, fov).
/// Each handle is its own `useStepDrag` instance: the hook is intentionally
/// thin so each call site stays free to clamp / wrap / scale the per-step
/// delta however it wants.
///
/// Pixel-to-step quantisation gives the same visual feedback the user gets
/// from the existing time/location scrubbers — values only change when the
/// pointer crosses a step boundary, so single-pixel jitter does not nudge
/// the value.

const DEFAULT_CLICK_SLOP_PX = 4;

export type StepDragHandlers = {
  onPointerDown: (event: ReactPointerEvent<HTMLElement>) => void;
  onPointerMove: (event: ReactPointerEvent<HTMLElement>) => void;
  onPointerUp: (event: ReactPointerEvent<HTMLElement>) => void;
  onPointerCancel: (event: ReactPointerEvent<HTMLElement>) => void;
};

type DragState<Base> = {
  pointerId: number;
  startX: number;
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

  const endDrag = (event: ReactPointerEvent<HTMLElement>) => {
    const drag = stateRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    if (drag.moved) suppressClickRef.current = true;
    if (drag.element.hasPointerCapture(event.pointerId)) {
      drag.element.releasePointerCapture(event.pointerId);
    }
    stateRef.current = null;
  };

  const handlers: StepDragHandlers = {
    onPointerDown: (event) => {
      if (event.button !== 0) return;
      const element = event.currentTarget;
      element.setPointerCapture(event.pointerId);
      stateRef.current = {
        pointerId: event.pointerId,
        startX: event.clientX,
        base: opts.onStart(),
        lastStep: 0,
        moved: false,
        element,
      };
    },
    onPointerMove: (event) => {
      const drag = stateRef.current;
      if (!drag || drag.pointerId !== event.pointerId) return;
      const deltaX = event.clientX - drag.startX;
      if (Math.abs(deltaX) >= slop) drag.moved = true;
      const steps = Math.trunc(deltaX / opts.pxPerStep);
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
