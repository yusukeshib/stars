import { describe, it, expect } from "vitest";
import { render, fireEvent, type RenderResult } from "@testing-library/react";
import axe, { type RunOptions, type Result } from "axe-core";
import type { ReactElement } from "react";

import { I18nProvider } from "../i18n";
import { OverlayToggles } from "../components/OverlayToggles";
import { TourPanel } from "../components/TourPanel";
import { DEFAULT_OVERLAY_CONFIG } from "../observer";
import { FIRST_NIGHT_TOUR } from "../tour";

/// L-24 automated accessibility gate.
///
/// Mounts the web frontend's interactive presentational components in jsdom
/// and runs axe-core (WCAG 2.2 A/AA rule sets) against the rendered DOM,
/// failing the build on any violation. This is the "automated axe-core CI
/// gate" the `L-24` accessibility pass deferred until the frontend gained a
/// JS test harness.
///
/// Two rule families are intentionally disabled under jsdom because they are
/// **page-level** or **layout-level** checks that cannot be evaluated for an
/// isolated component fragment mounted without the full document chrome:
///
///   - `color-contrast` — jsdom does no layout/paint, so computed colours and
///     contrast ratios are unavailable. Contrast is covered by the manual
///     screen-reader/contrast pass documented in ROADMAP `L-24`; a real-browser
///     Lighthouse contrast audit can be layered on later via Playwright.
///   - `region` / `landmark-*` / `page-has-heading-one` — these assert
///     whole-page landmark structure, which only makes sense for the full
///     `App`, not a single settings card or tour panel rendered in isolation.
///
/// Every other WCAG rule (form labels, accessible names, ARIA validity,
/// roles, duplicate ids, list semantics, focusable controls, etc.) runs.
const AXE_OPTIONS: RunOptions = {
  runOnly: { type: "tag", values: ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"] },
  rules: {
    "color-contrast": { enabled: false },
    region: { enabled: false },
  },
};

function formatViolations(violations: Result[]): string {
  return violations
    .map((v) => {
      const nodes = v.nodes.map((n) => `      - ${n.html}`).join("\n");
      return `  [${v.impact ?? "n/a"}] ${v.id}: ${v.help}\n    ${v.helpUrl}\n${nodes}`;
    })
    .join("\n\n");
}

async function expectNoViolations(container: HTMLElement): Promise<void> {
  const results = await axe.run(container, AXE_OPTIONS);
  if (results.violations.length > 0) {
    throw new Error(
      `axe-core found ${results.violations.length} accessibility violation(s):\n\n${formatViolations(
        results.violations,
      )}`,
    );
  }
  expect(results.violations).toEqual([]);
}

function renderWithI18n(ui: ReactElement): RenderResult {
  return render(<I18nProvider>{ui}</I18nProvider>);
}

describe("web frontend accessibility (axe-core, L-24)", () => {
  it("OverlayToggles settings card has no WCAG violations", async () => {
    const { container } = renderWithI18n(
      <OverlayToggles config={DEFAULT_OVERLAY_CONFIG} onChange={() => {}} />,
    );
    await expectNoViolations(container);
  });

  it("TourPanel launch button has no WCAG violations", async () => {
    const { container } = renderWithI18n(
      <TourPanel tour={FIRST_NIGHT_TOUR} onApplyScene={() => {}} />,
    );
    await expectNoViolations(container);
  });

  it("TourPanel active step has no WCAG violations", async () => {
    const { container, getByRole } = renderWithI18n(
      <TourPanel tour={FIRST_NIGHT_TOUR} onApplyScene={() => {}} />,
    );
    // Enter the guided-tour panel (step 0) so the stepper UI is exercised.
    fireEvent.click(getByRole("button", { name: /Start guided tour/i }));
    await expectNoViolations(container);
  });
});
