/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<ead03f06c21256e751e0055980145156>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/Libraries/Animated/AnimatedImplementation.js
 */

import type { EventConfig, Mapping } from "./AnimatedEvent";
import type { EndCallback } from "./animations/Animation";
import type { DecayAnimationConfig } from "./animations/DecayAnimation";
import type { SpringAnimationConfig } from "./animations/SpringAnimation";
import type { TimingAnimationConfig } from "./animations/TimingAnimation";
import { AnimatedEvent, attachNativeEventImpl } from "./AnimatedEvent";
import createAnimatedComponent from "./createAnimatedComponent";
import AnimatedAddition from "./nodes/AnimatedAddition";
import AnimatedColor from "./nodes/AnimatedColor";
import AnimatedDiffClamp from "./nodes/AnimatedDiffClamp";
import AnimatedDivision from "./nodes/AnimatedDivision";
import AnimatedInterpolation from "./nodes/AnimatedInterpolation";
import AnimatedModulo from "./nodes/AnimatedModulo";
import AnimatedMultiplication from "./nodes/AnimatedMultiplication";
import AnimatedNode from "./nodes/AnimatedNode";
import AnimatedSubtraction from "./nodes/AnimatedSubtraction";
import AnimatedValue from "./nodes/AnimatedValue";
import AnimatedValueXY from "./nodes/AnimatedValueXY";
export type CompositeAnimation = {
  start: (callback?: EndCallback | undefined, isLooping?: boolean) => void;
  stop: () => void;
  reset: () => void;
};
declare const addImpl: (a: AnimatedNode | number, b: AnimatedNode | number) => AnimatedAddition;
declare const subtractImpl: (a: AnimatedNode | number, b: AnimatedNode | number) => AnimatedSubtraction;
declare const divideImpl: (a: AnimatedNode | number, b: AnimatedNode | number) => AnimatedDivision;
declare const multiplyImpl: (a: AnimatedNode | number, b: AnimatedNode | number) => AnimatedMultiplication;
declare const moduloImpl: (a: AnimatedNode, modulus: number) => AnimatedModulo;
declare const diffClampImpl: (a: AnimatedNode, min: number, max: number) => AnimatedDiffClamp;
declare const springImpl: (value: AnimatedValue | AnimatedValueXY | AnimatedColor, config: SpringAnimationConfig) => CompositeAnimation;
declare const timingImpl: (value: AnimatedValue | AnimatedValueXY | AnimatedColor, config: TimingAnimationConfig) => CompositeAnimation;
declare const decayImpl: (value: AnimatedValue | AnimatedValueXY | AnimatedColor, config: DecayAnimationConfig) => CompositeAnimation;
declare const sequenceImpl: (animations: Array<CompositeAnimation>) => CompositeAnimation;
type ParallelConfig = {
  stopTogether?: boolean;
};
declare const parallelImpl: (animations: Array<CompositeAnimation>, config?: null | undefined | ParallelConfig) => CompositeAnimation;
declare const delayImpl: (time: number) => CompositeAnimation;
declare const staggerImpl: (time: number, animations: Array<CompositeAnimation>) => CompositeAnimation;
type LoopAnimationConfig = {
  iterations: number;
  resetBeforeIteration?: boolean;
};
declare const loopImpl: (animation: CompositeAnimation, $$PARAM_1$$?: LoopAnimationConfig) => CompositeAnimation;
declare function forkEventImpl(event: (null | undefined | AnimatedEvent) | (null | undefined | Function), listener: Function): AnimatedEvent | Function;
declare function unforkEventImpl(event: (null | undefined | AnimatedEvent) | (null | undefined | Function), listener: Function): void;
declare const eventImpl: <T>(argMapping: ReadonlyArray<null | undefined | Mapping>, config: EventConfig<T>) => any;
type AnimatedNumeric = AnimatedAddition | AnimatedDiffClamp | AnimatedDivision | AnimatedInterpolation<number> | AnimatedModulo | AnimatedMultiplication | AnimatedSubtraction | AnimatedValue;
export type { AnimatedNumeric as Numeric };
/**
 * The `Animated` library is designed to make animations fluid, powerful, and
 * easy to build and maintain. `Animated` focuses on declarative relationships
 * between inputs and outputs, with configurable transforms in between, and
 * simple `start`/`stop` methods to control time-based animation execution.
 * If additional transforms are added, be sure to include them in
 * AnimatedMock.js as well.
 *
 * See https://reactnative.dev/docs/animated
 */
declare const $$AnimatedImplementation: {
  /**
   * Standard value class for driving animations.  Typically initialized with
   * `new Animated.Value(0);`
   *
   * See https://reactnative.dev/docs/animated#value
   */
  Value: typeof AnimatedValue;
  /**
   * 2D value class for driving 2D animations, such as pan gestures.
   *
   * See https://reactnative.dev/docs/animatedvaluexy
   */
  ValueXY: typeof AnimatedValueXY;
  /**
   * Value class for driving color animations.
   */
  Color: typeof AnimatedColor;
  /**
   * Exported to use the Interpolation type in flow.
   *
   * See https://reactnative.dev/docs/animated#interpolation
   */
  Interpolation: typeof AnimatedInterpolation;
  /**
   * Exported for ease of type checking. All animated values derive from this
   * class.
   *
   * See https://reactnative.dev/docs/animated#node
   */
  Node: typeof AnimatedNode;
  decay: typeof decayImpl;
  timing: typeof timingImpl;
  spring: typeof springImpl;
  add: typeof addImpl;
  subtract: typeof subtractImpl;
  divide: typeof divideImpl;
  multiply: typeof multiplyImpl;
  modulo: typeof moduloImpl;
  diffClamp: typeof diffClampImpl;
  delay: typeof delayImpl;
  sequence: typeof sequenceImpl;
  parallel: typeof parallelImpl;
  stagger: typeof staggerImpl;
  loop: typeof loopImpl;
  event: typeof eventImpl;
  createAnimatedComponent: typeof createAnimatedComponent;
  attachNativeEvent: typeof attachNativeEventImpl;
  forkEvent: typeof forkEventImpl;
  unforkEvent: typeof unforkEventImpl;
  /**
   * Expose Event class, so it can be used as a type for type checkers.
   */
  Event: typeof AnimatedEvent;
};
declare type $$AnimatedImplementation = typeof $$AnimatedImplementation;
export default $$AnimatedImplementation;
