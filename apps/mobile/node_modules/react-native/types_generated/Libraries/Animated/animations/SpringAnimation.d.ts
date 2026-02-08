/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<b4097007f0fa633a18b89c525ea20c52>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/Libraries/Animated/animations/SpringAnimation.js
 */

import type AnimatedInterpolation from "../nodes/AnimatedInterpolation";
import type AnimatedValue from "../nodes/AnimatedValue";
import type AnimatedValueXY from "../nodes/AnimatedValueXY";
import type { AnimationConfig, EndCallback } from "./Animation";
import AnimatedColor from "../nodes/AnimatedColor";
import Animation from "./Animation";
export type SpringAnimationConfig = Readonly<Omit<AnimationConfig, keyof {
  toValue: number | AnimatedValue | {
    x: number;
    y: number;
  } | AnimatedValueXY | {
    r: number;
    g: number;
    b: number;
    a: number;
  } | AnimatedColor | AnimatedInterpolation<number>;
  overshootClamping?: boolean;
  restDisplacementThreshold?: number;
  restSpeedThreshold?: number;
  velocity?: number | Readonly<{
    x: number;
    y: number;
  }>;
  bounciness?: number;
  speed?: number;
  tension?: number;
  friction?: number;
  stiffness?: number;
  damping?: number;
  mass?: number;
  delay?: number;
}> & {
  toValue: number | AnimatedValue | {
    x: number;
    y: number;
  } | AnimatedValueXY | {
    r: number;
    g: number;
    b: number;
    a: number;
  } | AnimatedColor | AnimatedInterpolation<number>;
  overshootClamping?: boolean;
  restDisplacementThreshold?: number;
  restSpeedThreshold?: number;
  velocity?: number | Readonly<{
    x: number;
    y: number;
  }>;
  bounciness?: number;
  speed?: number;
  tension?: number;
  friction?: number;
  stiffness?: number;
  damping?: number;
  mass?: number;
  delay?: number;
}>;
export type SpringAnimationConfigSingle = Readonly<Omit<AnimationConfig, keyof {
  toValue: number;
  overshootClamping?: boolean;
  restDisplacementThreshold?: number;
  restSpeedThreshold?: number;
  velocity?: number;
  bounciness?: number;
  speed?: number;
  tension?: number;
  friction?: number;
  stiffness?: number;
  damping?: number;
  mass?: number;
  delay?: number;
}> & {
  toValue: number;
  overshootClamping?: boolean;
  restDisplacementThreshold?: number;
  restSpeedThreshold?: number;
  velocity?: number;
  bounciness?: number;
  speed?: number;
  tension?: number;
  friction?: number;
  stiffness?: number;
  damping?: number;
  mass?: number;
  delay?: number;
}>;
declare type SpringAnimationInternalState = symbol & {
  __SpringAnimationInternalState__: string;
};
declare class SpringAnimation extends Animation {
  constructor(config: SpringAnimationConfigSingle);
  start(fromValue: number, onUpdate: (value: number) => void, onEnd: null | undefined | EndCallback, previousAnimation: null | undefined | Animation, animatedValue: AnimatedValue): void;
  getInternalState(): SpringAnimationInternalState;
  /**
   * This spring model is based off of a damped harmonic oscillator
   * (https://en.wikipedia.org/wiki/Harmonic_oscillator#Damped_harmonic_oscillator).
   *
   * We use the closed form of the second order differential equation:
   *
   * x'' + (2ζ⍵_0)x' + ⍵^2x = 0
   *
   * where
   *    ⍵_0 = √(k / m) (undamped angular frequency of the oscillator),
   *    ζ = c / 2√mk (damping ratio),
   *    c = damping constant
   *    k = stiffness
   *    m = mass
   *
   * The derivation of the closed form is described in detail here:
   * http://planetmath.org/sites/default/files/texpdf/39745.pdf
   *
   * This algorithm happens to match the algorithm used by CASpringAnimation,
   * a QuartzCore (iOS) API that creates spring animations.
   */
  onUpdate(): void;
  stop(): void;
}
export default SpringAnimation;
