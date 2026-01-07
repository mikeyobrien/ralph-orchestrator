/**
 * Tests for UI Polish Components
 * Phase 5: Testing & Polish
 *
 * Note: These tests verify component rendering.
 * Animation behavior tested manually on device.
 */

import React from "react";
import renderer from "react-test-renderer";
import {
  Skeleton,
  SessionCardSkeleton,
  IterationSkeleton,
  MetricCardSkeleton,
} from "../components/Skeleton";
import { AnimatedCard, FadeIn, BounceIn } from "../components/AnimatedCard";
import { Text, View } from "react-native";

// Disable animation loop for testing
jest.mock("react-native", () => {
  const RN = jest.requireActual("react-native");
  RN.Animated.loop = jest.fn((animation) => ({
    start: jest.fn(),
    stop: jest.fn(),
  }));
  RN.Animated.timing = jest.fn(() => ({
    start: jest.fn(),
  }));
  RN.Animated.spring = jest.fn(() => ({
    start: jest.fn(),
  }));
  RN.Animated.parallel = jest.fn(() => ({
    start: jest.fn(),
  }));
  RN.Animated.sequence = jest.fn(() => ({
    start: jest.fn(),
    stop: jest.fn(),
  }));
  return RN;
});

describe("Skeleton Components", () => {
  it("Skeleton renders with default props", () => {
    const tree = renderer.create(<Skeleton />).toJSON();
    expect(tree).toBeTruthy();
  });

  it("Skeleton renders with custom dimensions", () => {
    const tree = renderer
      .create(<Skeleton width={200} height={40} borderRadius={8} />)
      .toJSON();
    expect(tree).toBeTruthy();
  });

  it("Skeleton renders with percentage width", () => {
    const tree = renderer
      .create(<Skeleton width="50%" height={20} />)
      .toJSON();
    expect(tree).toBeTruthy();
  });

  it("Skeleton renders without animation", () => {
    const tree = renderer.create(<Skeleton animated={false} />).toJSON();
    expect(tree).toBeTruthy();
  });

  it("SessionCardSkeleton renders", () => {
    const tree = renderer.create(<SessionCardSkeleton />).toJSON();
    expect(tree).toBeTruthy();
  });

  it("IterationSkeleton renders", () => {
    const tree = renderer.create(<IterationSkeleton />).toJSON();
    expect(tree).toBeTruthy();
  });

  it("MetricCardSkeleton renders", () => {
    const tree = renderer.create(<MetricCardSkeleton />).toJSON();
    expect(tree).toBeTruthy();
  });
});

describe("Animation Components", () => {
  it("AnimatedCard renders children", () => {
    const tree = renderer
      .create(
        <AnimatedCard>
          <Text>Test Content</Text>
        </AnimatedCard>
      )
      .toJSON();
    expect(tree).toBeTruthy();
  });

  it("AnimatedCard renders with delay", () => {
    const tree = renderer
      .create(
        <AnimatedCard delay={100}>
          <Text>Delayed</Text>
        </AnimatedCard>
      )
      .toJSON();
    expect(tree).toBeTruthy();
  });

  it("AnimatedCard renders with stagger index", () => {
    const tree = renderer
      .create(
        <AnimatedCard index={2} staggerDelay={50}>
          <Text>Staggered</Text>
        </AnimatedCard>
      )
      .toJSON();
    expect(tree).toBeTruthy();
  });

  it("AnimatedCard pressable mode", () => {
    const mockPress = jest.fn();
    const tree = renderer
      .create(
        <AnimatedCard pressable onPress={mockPress}>
          <Text>Pressable</Text>
        </AnimatedCard>
      )
      .toJSON();
    expect(tree).toBeTruthy();
  });

  it("FadeIn renders children", () => {
    const tree = renderer
      .create(
        <FadeIn>
          <Text>Fading</Text>
        </FadeIn>
      )
      .toJSON();
    expect(tree).toBeTruthy();
  });

  it("FadeIn with delay", () => {
    const tree = renderer
      .create(
        <FadeIn delay={200}>
          <Text>Delayed Fade</Text>
        </FadeIn>
      )
      .toJSON();
    expect(tree).toBeTruthy();
  });

  it("BounceIn renders children", () => {
    const tree = renderer
      .create(
        <BounceIn>
          <Text>Bouncing</Text>
        </BounceIn>
      )
      .toJSON();
    expect(tree).toBeTruthy();
  });

  it("BounceIn with delay", () => {
    const tree = renderer
      .create(
        <BounceIn delay={100}>
          <Text>Delayed Bounce</Text>
        </BounceIn>
      )
      .toJSON();
    expect(tree).toBeTruthy();
  });
});

describe("Component Integration", () => {
  it("AnimatedCard with SessionCardSkeleton", () => {
    const tree = renderer
      .create(
        <AnimatedCard>
          <SessionCardSkeleton />
        </AnimatedCard>
      )
      .toJSON();
    expect(tree).toBeTruthy();
  });

  it("Multiple AnimatedCards with stagger", () => {
    const items = ["A", "B", "C"];
    const tree = renderer
      .create(
        <View>
          {items.map((item, i) => (
            <AnimatedCard key={item} index={i} staggerDelay={50}>
              <Text>{item}</Text>
            </AnimatedCard>
          ))}
        </View>
      )
      .toJSON();
    expect(tree).toBeTruthy();
  });
});
