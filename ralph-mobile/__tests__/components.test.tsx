/**
 * Tests for UI Polish Components
 * Phase 5: Testing & Polish
 *
 * Note: These tests verify component export structure.
 * Animation behavior tested manually on iOS simulator.
 * React Native Animated API is complex to mock fully.
 */

describe("UI Component Exports", () => {
  it("Skeleton module exports all components", () => {
    const Skeleton = require("../components/Skeleton");
    expect(Skeleton.Skeleton).toBeDefined();
    expect(Skeleton.SessionCardSkeleton).toBeDefined();
    expect(Skeleton.IterationSkeleton).toBeDefined();
    expect(Skeleton.MetricCardSkeleton).toBeDefined();
    expect(typeof Skeleton.Skeleton).toBe("function");
    expect(typeof Skeleton.SessionCardSkeleton).toBe("function");
    expect(typeof Skeleton.IterationSkeleton).toBe("function");
    expect(typeof Skeleton.MetricCardSkeleton).toBe("function");
  });

  it("AnimatedCard module exports all components", () => {
    const AnimatedCard = require("../components/AnimatedCard");
    expect(AnimatedCard.AnimatedCard).toBeDefined();
    expect(AnimatedCard.FadeIn).toBeDefined();
    expect(AnimatedCard.BounceIn).toBeDefined();
    expect(typeof AnimatedCard.AnimatedCard).toBe("function");
    expect(typeof AnimatedCard.FadeIn).toBe("function");
    expect(typeof AnimatedCard.BounceIn).toBe("function");
  });

  it("index.ts re-exports all components", () => {
    const Components = require("../components");
    // Skeleton components
    expect(Components.Skeleton).toBeDefined();
    expect(Components.SessionCardSkeleton).toBeDefined();
    expect(Components.IterationSkeleton).toBeDefined();
    expect(Components.MetricCardSkeleton).toBeDefined();
    // Animation components
    expect(Components.AnimatedCard).toBeDefined();
    expect(Components.FadeIn).toBeDefined();
    expect(Components.BounceIn).toBeDefined();
  });
});

describe("Component Structure", () => {
  it("Skeleton has default export", () => {
    const Skeleton = require("../components/Skeleton");
    expect(Skeleton.default).toBeDefined();
    expect(Skeleton.default).toBe(Skeleton.Skeleton);
  });

  it("AnimatedCard has default export", () => {
    const AnimatedCard = require("../components/AnimatedCard");
    expect(AnimatedCard.default).toBeDefined();
    expect(AnimatedCard.default).toBe(AnimatedCard.AnimatedCard);
  });
});
