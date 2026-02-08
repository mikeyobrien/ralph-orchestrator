import { it, expect, vi } from 'vitest';

import { Source, Sink, Operator, Signal, SignalKind, TalkbackKind, TalkbackFn } from '../types';
import { push, start } from '../helpers';

/* This tests a noop operator for passive Pull talkback signals.
  A Pull will be sent from the sink upwards and should pass through
  the operator until the source receives it, which then pushes a
  value down. */
export const passesPassivePull = (operator: Operator<any, any>, output: any = 0) => {
  it('responds to Pull talkback signals (spec)', () => {
    let talkback: TalkbackFn | null = null;
    let pushes = 0;
    const values: any[] = [];

    const source: Source<any> = sink => {
      sink(
        start(signal => {
          if (!pushes && signal === TalkbackKind.Pull) {
            pushes++;
            sink(push(0));
          }
        })
      );
    };

    const sink: Sink<any> = signal => {
      expect(signal).not.toBe(SignalKind.End);
      if (signal === SignalKind.End) {
        /*noop*/
      } else if (signal.tag === SignalKind.Push) {
        values.push(signal[0]);
      } else {
        talkback = signal[0];
      }
    };

    operator(source)(sink);
    // The Start signal should always come in immediately
    expect(talkback).not.toBe(null);
    // No Push signals should be issued initially
    expect(values).toEqual([]);

    // When pulling a value we expect an immediate response
    talkback!(TalkbackKind.Pull);
    vi.runAllTimers();
    expect(values).toEqual([output]);
  });
};

/* This tests a noop operator for regular, active Push signals.
  A Push will be sent downwards from the source, through the
  operator to the sink. Pull events should be let through from
  the sink after every Push event. */
export const passesActivePush = (operator: Operator<any, any>, result: any = 0) => {
  it('responds to eager Push signals (spec)', () => {
    const values: any[] = [];
    let talkback: TalkbackFn | null = null;
    let sink: Sink<any> | null = null;
    let pulls = 0;

    const source: Source<any> = _sink => {
      (sink = _sink)(
        start(signal => {
          if (signal === TalkbackKind.Pull) pulls++;
        })
      );
    };

    operator(source)(signal => {
      expect(signal).not.toBe(SignalKind.End);
      if (signal === SignalKind.End) {
        /*noop*/
      } else if (signal.tag === SignalKind.Start) {
        talkback = signal[0];
      } else if (signal.tag === SignalKind.Push) {
        values.push(signal[0]);
        talkback!(TalkbackKind.Pull);
      }
    });

    // No Pull signals should be issued initially
    expect(pulls).toBe(0);

    // When pushing a value we expect an immediate response
    sink!(push(0));
    vi.runAllTimers();
    expect(values).toEqual([result]);
    // Subsequently the Pull signal should have travelled upwards
    expect(pulls).toBe(1);
  });
};

/* This tests a noop operator for Close talkback signals from the sink.
  A Close signal will be sent, which should be forwarded to the source,
  which then ends the communication without sending an End signal. */
export const passesSinkClose = (operator: Operator<any, any>) => {
  it('responds to Close signals from sink (spec)', () => {
    let talkback: TalkbackFn | null = null;
    let closing = 0;

    const source: Source<any> = sink => {
      sink(
        start(signal => {
          if (signal === TalkbackKind.Pull && !closing) {
            sink(push(0));
          } else if (signal === TalkbackKind.Close) {
            closing++;
          }
        })
      );
    };

    const sink: Sink<any> = signal => {
      expect(signal).not.toBe(SignalKind.End);
      if (signal === SignalKind.End) {
        /*noop*/
      } else if (signal.tag === SignalKind.Push) {
        talkback!(TalkbackKind.Close);
      } else {
        talkback = signal[0];
      }
    };

    operator(source)(sink);

    // When pushing a value we expect an immediate close signal
    talkback!(TalkbackKind.Pull);
    vi.runAllTimers();
    expect(closing).toBe(1);
  });
};

/* This tests a noop operator for End signals from the source.
  A Push and End signal will be sent after the first Pull talkback
  signal from the sink, which shouldn't lead to any extra Close or Pull
  talkback signals. */
export const passesSourceEnd = (operator: Operator<any, any>, result: any = 0) => {
  it('passes on immediate Push then End signals from source (spec)', () => {
    const signals: Signal<any>[] = [];
    let talkback: TalkbackFn | null = null;
    let pulls = 0;
    let ending = 0;

    const source: Source<any> = sink => {
      sink(
        start(signal => {
          expect(signal).not.toBe(TalkbackKind.Close);
          if (signal === TalkbackKind.Pull) {
            pulls++;
            if (pulls === 1) {
              sink(push(0));
              sink(SignalKind.End);
            }
          }
        })
      );
    };

    const sink: Sink<any> = signal => {
      if (signal === SignalKind.End) {
        signals.push(signal);
        ending++;
      } else if (signal.tag === SignalKind.Push) {
        signals.push(signal);
      } else {
        talkback = signal[0];
      }
    };

    operator(source)(sink);

    // When pushing a value we expect an immediate Push then End signal
    talkback!(TalkbackKind.Pull);
    vi.runAllTimers();
    expect(ending).toBe(1);
    expect(signals).toEqual([push(result), SignalKind.End]);
    // Also no additional pull event should be created by the operator
    expect(pulls).toBe(1);
  });
};

/* This tests a noop operator for End signals from the source
  after the first pull in response to another.
  This is similar to passesSourceEnd but more well behaved since
  mergeMap/switchMap/concatMap are eager operators. */
export const passesSourcePushThenEnd = (operator: Operator<any, any>, result: any = 0) => {
  it('passes on End signals from source (spec)', () => {
    const signals: Signal<any>[] = [];
    let talkback: TalkbackFn | null = null;
    let pulls = 0;
    let ending = 0;

    const source: Source<any> = sink => {
      sink(
        start(signal => {
          expect(signal).not.toBe(TalkbackKind.Close);
          if (signal === TalkbackKind.Pull) {
            pulls++;
            if (pulls <= 2) {
              sink(push(0));
            } else {
              sink(SignalKind.End);
            }
          }
        })
      );
    };

    const sink: Sink<any> = signal => {
      if (signal === SignalKind.End) {
        signals.push(signal);
        ending++;
      } else if (signal.tag === SignalKind.Push) {
        signals.push(signal);
        talkback!(TalkbackKind.Pull);
      } else {
        talkback = signal[0];
      }
    };

    operator(source)(sink);

    // When pushing a value we expect an immediate Push then End signal
    talkback!(TalkbackKind.Pull);
    vi.runAllTimers();
    expect(ending).toBe(1);
    expect(pulls).toBe(3);
    expect(signals).toEqual([push(result), push(result), SignalKind.End]);
  });
};

/* This tests a noop operator for Start signals from the source.
  When the operator's sink is started by the source it'll receive
  a Start event. As a response it should never send more than one
  Start signals to the sink. */
export const passesSingleStart = (operator: Operator<any, any>) => {
  it('sends a single Start event to the incoming sink (spec)', () => {
    let starts = 0;

    const source: Source<any> = sink => {
      sink(start(() => {}));
    };

    const sink: Sink<any> = signal => {
      if (signal !== SignalKind.End && signal.tag === SignalKind.Start) {
        starts++;
      }
    };

    // When starting the operator we expect a single start event on the sink
    operator(source)(sink);
    expect(starts).toBe(1);
  });
};

/* This tests a noop operator for silence after End signals from the source.
  When the operator receives the End signal it shouldn't forward any other
  signals to the sink anymore.
  This isn't a strict requirement, but some operators should ensure that
  all sources are well behaved. This is particularly true for operators
  that either Close sources themselves or may operate on multiple sources. */
export const passesStrictEnd = (operator: Operator<any, any>) => {
  it('stops all signals after End has been received (spec: strict end)', () => {
    let pulls = 0;
    const signals: Signal<any>[] = [];

    const source: Source<any> = sink => {
      sink(
        start(signal => {
          if (signal === TalkbackKind.Pull) {
            pulls++;
            sink(SignalKind.End);
            sink(push(123));
          }
        })
      );
    };

    const sink: Sink<any> = signal => {
      if (signal === SignalKind.End) {
        signals.push(signal);
      } else if (signal.tag === SignalKind.Push) {
        signals.push(signal);
      } else {
        signal[0](TalkbackKind.Pull);
      }
    };

    operator(source)(sink);

    // The Push signal should've been dropped
    vi.runAllTimers();
    expect(signals).toEqual([SignalKind.End]);
    expect(pulls).toBe(1);
  });

  it('stops all signals after Close has been received (spec: strict close)', () => {
    const signals: Signal<any>[] = [];

    const source: Source<any> = sink => {
      sink(
        start(signal => {
          if (signal === TalkbackKind.Close) {
            sink(push(123));
          }
        })
      );
    };

    const sink: Sink<any> = signal => {
      if (signal === SignalKind.End) {
        signals.push(signal);
      } else if (signal.tag === SignalKind.Push) {
        signals.push(signal);
      } else {
        signal[0](TalkbackKind.Close);
      }
    };

    operator(source)(sink);

    // The Push signal should've been dropped
    vi.runAllTimers();
    expect(signals).toEqual([]);
  });
};

/* This tests an immediately closing operator for End signals to
  the sink and Close signals to the source.
  When an operator closes immediately we expect to see a Close
  signal at the source and an End signal to the sink, since the
  closing operator is expected to end the entire chain. */
export const passesCloseAndEnd = (closingOperator: Operator<any, any>) => {
  it('closes the source and ends the sink correctly (spec: ending operator)', () => {
    let closing = 0;
    let ending = 0;

    const source: Source<any> = sink => {
      sink(
        start(signal => {
          // For some operator tests we do need to send a single value
          if (signal === TalkbackKind.Pull) {
            sink(push(null));
          } else {
            closing++;
          }
        })
      );
    };

    const sink: Sink<any> = signal => {
      if (signal === SignalKind.End) {
        ending++;
      } else if (signal.tag === SignalKind.Start) {
        signal[0](TalkbackKind.Pull);
      }
    };

    // We expect the operator to immediately end and close
    closingOperator(source)(sink);
    expect(closing).toBe(1);
    expect(ending).toBe(1);
  });
};

export const passesAsyncSequence = (operator: Operator<any, any>, result: any = 0) => {
  it('passes an async push with an async end (spec)', () => {
    let hasPushed = false;
    const signals: Signal<any>[] = [];

    const source: Source<any> = sink => {
      sink(
        start(signal => {
          if (signal === TalkbackKind.Pull && !hasPushed) {
            hasPushed = true;
            setTimeout(() => sink(push(0)), 10);
            setTimeout(() => sink(SignalKind.End), 20);
          }
        })
      );
    };

    const sink: Sink<any> = signal => {
      if (signal === SignalKind.End) {
        signals.push(signal);
      } else if (signal.tag === SignalKind.Push) {
        signals.push(signal);
      } else {
        setTimeout(() => {
          signal[0](TalkbackKind.Pull);
        }, 5);
      }
    };

    // We initially expect to see the push signal
    // Afterwards after all timers all other signals come in
    operator(source)(sink);
    expect(signals.length).toBe(0);
    vi.advanceTimersByTime(5);
    expect(hasPushed).toBeTruthy();
    vi.runAllTimers();

    expect(signals).toEqual([push(result), SignalKind.End]);
  });
};
