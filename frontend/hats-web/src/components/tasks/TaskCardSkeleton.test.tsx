/**
 * TaskCardSkeleton Component Tests
 *
 * Tests for the skeleton loading placeholder that matches
 * the two-row TaskCard structure. No animation per spec,
 * just static gray placeholder rectangles.
 */

import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { TaskCardSkeleton } from "./TaskCardSkeleton";

describe("TaskCardSkeleton", () => {
  describe("structure", () => {
    it("renders two rows matching TaskCard layout", () => {
      render(<TaskCardSkeleton />);

      // Should have a card-like container
      const skeleton = document.querySelector("[data-testid='task-card-skeleton']");
      expect(skeleton).toBeInTheDocument();

      // Should have two row containers
      const rows = skeleton?.querySelectorAll("[data-testid^='skeleton-row']");
      expect(rows?.length).toBe(2);
    });

    it("row 1 has icon placeholder and title bar", () => {
      render(<TaskCardSkeleton />);

      const row1 = document.querySelector("[data-testid='skeleton-row-1']");
      expect(row1).toBeInTheDocument();

      // Icon placeholder (small square)
      const iconPlaceholder = row1?.querySelector("[data-testid='skeleton-icon']");
      expect(iconPlaceholder).toBeInTheDocument();

      // Title bar (longer rectangle)
      const titlePlaceholder = row1?.querySelector("[data-testid='skeleton-title']");
      expect(titlePlaceholder).toBeInTheDocument();
    });

    it("row 2 has badge placeholders and time placeholder", () => {
      render(<TaskCardSkeleton />);

      const row2 = document.querySelector("[data-testid='skeleton-row-2']");
      expect(row2).toBeInTheDocument();

      // Badge placeholders (2-3 small rectangles)
      const badgePlaceholders = row2?.querySelectorAll("[data-testid^='skeleton-badge']");
      expect(badgePlaceholders?.length).toBeGreaterThanOrEqual(2);

      // Time placeholder on the right
      const timePlaceholder = row2?.querySelector("[data-testid='skeleton-time']");
      expect(timePlaceholder).toBeInTheDocument();
    });
  });

  describe("styling", () => {
    it("uses bg-muted for placeholder rectangles", () => {
      render(<TaskCardSkeleton />);

      // All placeholder elements should have bg-muted
      const placeholders = document.querySelectorAll("[data-testid^='skeleton-']");
      const muted = Array.from(placeholders).filter(
        (el) => el.classList.contains("bg-muted") || el.classList.contains("bg-muted/50")
      );
      // At least the main placeholders should be muted
      expect(muted.length).toBeGreaterThan(0);
    });

    it("does NOT have animation classes (per spec)", () => {
      render(<TaskCardSkeleton />);

      // Should NOT have animate-pulse or similar animation classes
      const skeleton = document.querySelector("[data-testid='task-card-skeleton']");
      expect(skeleton).not.toHaveClass("animate-pulse");

      // Check all children for animation
      const allElements = skeleton?.querySelectorAll("*") ?? [];
      for (const el of allElements) {
        expect(el).not.toHaveClass("animate-pulse");
        expect(el).not.toHaveClass("animate-shimmer");
      }
    });

    it("matches card dimensions approximately", () => {
      render(<TaskCardSkeleton />);

      const skeleton = document.querySelector("[data-testid='task-card-skeleton']");
      // Should have rounded corners like cards
      expect(skeleton).toHaveClass("rounded-lg");
    });

    it("applies custom className", () => {
      render(<TaskCardSkeleton className="custom-skeleton" />);

      const skeleton = document.querySelector(".custom-skeleton");
      expect(skeleton).toBeInTheDocument();
    });
  });

  describe("accessibility", () => {
    it("has aria-hidden for screen readers", () => {
      render(<TaskCardSkeleton />);

      const skeleton = document.querySelector("[data-testid='task-card-skeleton']");
      expect(skeleton).toHaveAttribute("aria-hidden", "true");
    });
  });

  describe("multiple skeletons", () => {
    it("renders multiple skeletons for loading lists", () => {
      render(
        <div>
          {[1, 2, 3].map((i) => (
            <TaskCardSkeleton key={i} />
          ))}
        </div>
      );

      const skeletons = document.querySelectorAll("[data-testid='task-card-skeleton']");
      expect(skeletons.length).toBe(3);
    });
  });
});
