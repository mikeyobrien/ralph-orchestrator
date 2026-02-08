/**
 * Talkback signal that sends instructions from a sink to a source.
 *
 * @remarks
 * This signal is issued via {@link TalkbackFn | talkback functions} that a {@link Sink} receives via
 * the {@link Start} signal, to tell a {@link Source} to either send a new value (pulling) or stop
 * sending values altogether (cancellation).
 */
export declare enum TalkbackKind {
  /** Instructs the {@link Source} to send the next value. */
  Pull = 0,
  /** Instructs the {@link Source} to stop sending values and cancels it. */
  Close = 1,
}

/**
 * Talkback callback that sends instructions to a source.
 *
 * @remarks
 * This function sends a {@link TalkbackKind} signal to the source to instruct it to send a new value
 * (pulling) or to be cancelled and stop sending values altogether.
 */
export type TalkbackFn = (signal: TalkbackKind) => void;

/**
 * Callback that is called when a source is cancelled.
 *
 * @remarks
 * This is used, in particular, in the {@link make | make Source} and is a returned function that is
 * called when the {@link TalkbackKind.Close} signal is received by the source.
 */
export type TeardownFn = () => void;

/**
 * Tag enum that is used to on signals that are sent from a source to a sink.
 *
 * @remarks
 * This signal is issued by a {@link Source} and {@link Sink | Sinks} are called with it. The signals
 * carrying values ({@link Start} and {@link Push}) are sent as a unary `[T]` tuple tagged with
 * {@link Tag}. The {@link End} signal carries no value and is sent as a raw `0` value.
 * @see {@link Start} for the data structure of the start signal.
 * @see {@link Push} for the data structure of the push signal, carrying values.
 */
export declare enum SignalKind {
  /**
   * Informs the {@link Sink} that it's being called by a {@link Source}.
   *
   * @remarks
   * This starts the stream of values and carries a {@link TalkbackFn | talkback function} with it
   * that is used by the {@link Sink} to communicate back to the {@link Source}.
   * @see {@link Start} for the data structure of the signal.
   */
  Start = 0,
  /**
   * Informs the {@link Sink} of a new values that's incoming from the {@link Source}.
   *
   * @remarks
   * This informs the {@link Sink} of new values that are sent by the {@link Source}.
   * @see {@link Push} for the data structure of the signal.
   */
  Push = 1,
  /**
   * Informs the {@link Sink} that the {@link Source} has ended and that it won't send more values.
   *
   * @remarks
   * This signal signifies that the stream has stopped and that no more values are expected. Some
   * sources don't have a set end or limit on how many values will be sent. This signal is not sent
   * when the {@link Source} is cancelled with a {@link TalkbackKind.Close | Close talkback signal}.
   */
  End = 0,
}

/**
 * The tag property that's put on unary `[T]` tuple to turn them into signals carrying values.
 *
 * @internal
 */
export interface Tag<T> {
  tag: T;
}

/**
 * Indicates the start of a stream to a {@link Sink}.
 *
 * @remarks
 * This signal is sent from a {@link Source} to a {@link Sink} at the start of a stream to inform it
 * that values can be pulled and/or will be sent. This signal carries a
 * {@link TalkbackFn | talkback function} that is used by the {@link Sink} to communicate back to the
 * {@link Source} as a callback. The talkback accepts {@link TalkbackKind.Pull | Pull} and
 * {@link TalkbackKind.Close | Close} signals.
 */
export type Start<_T> = Tag<SignalKind.Start> & [TalkbackFn];

/**
 * Sends a new value to a {@link Sink}.
 *
 * @remarks
 * This signal is sent from a {@link Source} to a {@link Sink} to send a new value to it. This is
 * essentially the signal that wraps new values coming in, like an event. Values are carried on
 * unary tuples and can be accessed using `signal[0]`.
 */
export type Push<T> = Tag<SignalKind.Push> & [T];

/**
 * Signals are sent from {@link Source | Sources} to {@link Sink | Sinks} to inform them of changes.
 *
 * @remarks
 * A {@link Source}, when consumed, sends a sequence of events to {@link Sink | Sinks}. In order, a
 * {@link SignalKind.Start | Start} signal will always be sent first, followed optionally by one or
 * more {@link SignalKind.Push | Push signals}, carrying values and representing the stream. A
 * {@link Source} will send the {@link SignalKind.End | End signal} when it runs out of values. The
 * End signal will be omitted if the Source is cancelled by a
 * {@link TalkbackKind.Close | Close signal}, sent back from the {@link Sink}.
 * @see {@link SignalKind} for the kinds signals sent by {@link Source | Sources}.
 * @see {@link Start} for the data structure of the start signal.
 * @see {@link Push} for the data structure of the push signal.
 */
export type Signal<T> = Start<T> | Push<T> | SignalKind.End;

/**
 * Callback function that is called by a {@link Source} with {@link Signal | Signals}.
 *
 * @remarks
 * A Sink is a function that is called repeatedly with signals from a {@link Source}. It represents
 * the receiver of the stream of signals/events coming from a {@link Source}.
 * @see {@link Signal} for the data structure of signals.
 */
export type Sink<T> = (signal: Signal<T>) => void;

/** Factory function that calls {@link Sink | Sinks} with {@link Signal | Signals} when invoked.
 * @remarks
 * A Source is a factory function that when invoked with a {@link Sink}, calls it with
 * {@link Signal | Signals} to create a stream of events, informing it of new values and the
 * potential end of the stream of values. The first signal a Source sends is always a
 * {@link Start | Start signal} that sends a talkback function to the {@link Sink}, so it may request
 * new values or cancel the source.
 *
 * @see {@link Signal} for the data structure of signals.
 * @see {@link Sink} for the data structure of sinks.
 */
export type Source<T> = (sink: Sink<T>) => void;

/** Transform function that accepts a {@link Source} and returns a new one.
 * @remarks
 * Wonka comes with several helper operators that transform a given {@link Source} into a new one,
 * potentially changing its outputs, or the outputs' timing. An "operator" in Wonka typically
 * accepts arguments and then returns this kind of function, so they can be chained and composed.
 *
 * @see {@link pipe | `pipe`} for the helper used to compose operators.
 */
export type Operator<In, Out> = (a: Source<In>) => Source<Out>;

/** Type utility to determine the type of a {@link Source}. */
export type TypeOfSource<T> = T extends Source<infer U> ? U : never;

/** Subscription object that can be used to cancel a {@link Source}.
 * @see {@link subscribe | subscribe sink} for a helper that returns this structure.
 */
export interface Subscription {
  /**
   * Cancels a {@link Source} to stop the subscription from receiving new values.
   *
   * @see {@link TalkbackKind.Close | Close signal} This uses the {@link TalkbackFn | talkback function} to send a {@link TalkbackKind.Close | Close signal}
   *   to the subscribed-to {@link Source} to stop it from sending new values. This cleans up the subscription
   *   and ends it immediately.
   */
  unsubscribe(): void;
}

/** An Observer represents sending signals manually to a {@link Sink}.
 * @remarks
 * The Observer is used whenever a utility allows for signals to be sent manually as a {@link Source}
 * would send them.
 *
 * @see {@link make | `make` source} for a helper that uses this structure.
 */
export interface Observer<T> {
  /** Sends a new value to the receiving Sink.
   * @remarks
   * This creates a {@link Push | Push signal} that is sent to a {@link Sink}.
   */
  next(value: T): void;
  /** Indicates to the receiving Sink that no more values will be sent.
   * @remarks
   * This creates an {@link SignalKind.End | End signal} that is sent to a {@link Sink}. The Observer
   * will accept no more values via {@link Observer.next | `next` calls} once this method has been
   * invoked.
   */
  complete(): void;
}

/** Subjects combine a {@link Source} with the {@link Observer} that is used to send values on said Source.
 * @remarks
 * A Subject is used whenever an event hub-like structure is needed, as it both provides the
 * {@link Observer}'s methods to send signals, as well as the `source` to receive said signals.
 *
 * @see {@link makeSubject | `makeSubject` source} for a helper that creates this structure.
 */
export interface Subject<T> extends Observer<T> {
  /** The {@link Source} that issues the signals as the {@link Observer} methods are called. */
  source: Source<T>;
}

/** Async Iterable/Iterator after having converted a {@link Source}.
 * @see {@link toAsyncIterable} for a helper that creates this structure.
 */
export interface SourceIterable<T> extends AsyncIterator<T>, AsyncIterable<T> {}
