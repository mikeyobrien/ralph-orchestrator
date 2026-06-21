/**
 * Tests for the role metadata helpers.
 *
 * The only test here is for the "unknown role" fallback, because losing it
 * would crash HatNode (which reads `getRoleMeta(...).emoji` unconditionally)
 * rather than just producing wrong colors.
 */

import { describe, expect, it } from "vitest";
import { ROLE_META, getRoleMeta } from "./roles";

describe("getRoleMeta", () => {
  it("returns the custom fallback for unknown roles", () => {
    expect(getRoleMeta("unknown-role")).toBe(ROLE_META.custom);
  });
});
