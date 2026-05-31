import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";

// Unmount React trees and clear the jsdom document between tests so each axe
// scan sees only the component under test.
afterEach(() => {
  cleanup();
});
