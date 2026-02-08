import { describe, it, expect, vi } from 'vitest';

import { Source, Sink, SignalKind, TalkbackKind } from '../types';
import { push, start } from '../helpers';

import * as sinks from '../sinks';
import * as sources from '../sources';
import * as callbag from '../callbag';
import * as observable from '../observable';

import Observable from 'zen-observable';
import callbagIterate from 'callbag-iterate';
import callbagTake from 'callbag-take';

describe('subscribe', () => {
  it('sends Pull talkback signals every Push signal', () => {
    let pulls = 0;
    const fn = vi.fn();

    const source: Source<any> = sink => {
      sink(
        start(signal => {
          if (signal === TalkbackKind.Pull) {
            if (pulls < 3) {
              pulls++;
              sink(push(0));
            } else {
              sink(SignalKind.End);
              expect(pulls).toBe(3);
            }
          }
        })
      );
    };

    sinks.subscribe(fn)(source);
    expect(fn).toHaveBeenCalledTimes(3);
    expect(pulls).toBe(3);
  });

  it('cancels when unsubscribe is called', () => {
    let pulls = 0;
    let closing = 0;

    const source: Source<any> = sink => {
      sink(
        start(signal => {
          if (signal === TalkbackKind.Pull) {
            if (!pulls) {
              pulls++;
              sink(push(0));
            }
          } else {
            closing++;
          }
        })
      );
    };

    const sub = sinks.subscribe(() => {})(source);
    expect(pulls).toBe(1);

    sub.unsubscribe();
    expect(closing).toBe(1);
  });

  it('ignores cancellation when the source has already ended', () => {
    let pulls = 0;
    let closing = 0;

    const source: Source<any> = sink => {
      sink(
        start(signal => {
          if (signal === TalkbackKind.Pull) {
            pulls++;
            sink(SignalKind.End);
          } else {
            closing++;
          }
        })
      );
    };

    const sub = sinks.subscribe(() => {})(source);
    expect(pulls).toBe(1);
    sub.unsubscribe();
    expect(closing).toBe(0);
  });

  it('ignores Push signals after the source has ended', () => {
    const fn = vi.fn();
    const source: Source<any> = sink => {
      sink(
        start(signal => {
          if (signal === TalkbackKind.Pull) {
            sink(SignalKind.End);
            sink(push(0));
          }
        })
      );
    };

    sinks.subscribe(fn)(source);
    expect(fn).not.toHaveBeenCalled();
  });

  it('ignores Push signals after cancellation', () => {
    const fn = vi.fn();
    const source: Source<any> = sink => {
      sink(
        start(signal => {
          if (signal === TalkbackKind.Close) {
            sink(push(0));
          }
        })
      );
    };

    sinks.subscribe(fn)(source).unsubscribe();
    expect(fn).not.toHaveBeenCalled();
  });
});

describe('publish', () => {
  it('sends Pull talkback signals every Push signal', () => {
    let pulls = 0;
    const source: Source<any> = sink => {
      sink(
        start(signal => {
          if (signal === TalkbackKind.Pull) {
            if (pulls < 3) {
              pulls++;
              sink(push(0));
            } else {
              sink(SignalKind.End);
              expect(pulls).toBe(3);
            }
          }
        })
      );
    };

    sinks.publish(source);
    expect(pulls).toBe(3);
  });
});

describe('toArray', () => {
  it('sends Pull talkback signals every Push signal', () => {
    let pulls = 0;
    const source: Source<any> = sink => {
      sink(
        start(signal => {
          if (signal === TalkbackKind.Pull) {
            if (pulls < 3) {
              pulls++;
              sink(push(0));
            } else {
              sink(SignalKind.End);
              expect(pulls).toBe(3);
            }
          }
        })
      );
    };

    const array = sinks.toArray(source);
    expect(array).toEqual([0, 0, 0]);
    expect(pulls).toBe(3);
  });

  it('sends a Close talkback signal after all synchronous values have been pulled', () => {
    let pulls = 0;
    let ending = 0;

    const source: Source<any> = sink => {
      sink(
        start(signal => {
          if (signal === TalkbackKind.Pull) {
            if (!pulls) {
              pulls++;
              sink(push(0));
            }
          } else {
            ending++;
          }
        })
      );
    };

    const array = sinks.toArray(source);
    expect(array).toEqual([0]);
    expect(ending).toBe(1);
  });
});

describe('toPromise', () => {
  it('creates a Promise that resolves on the last value', async () => {
    let pulls = 0;
    let sink: Sink<any> | null = null;

    const source: Source<any> = _sink => {
      sink = _sink;
      sink(
        start(signal => {
          if (signal === TalkbackKind.Pull) pulls++;
        })
      );
    };

    const fn = vi.fn();
    const promise = sinks.toPromise(source).then(fn);

    expect(pulls).toBe(1);
    sink!(push(0));
    expect(pulls).toBe(2);
    sink!(push(1));
    sink!(SignalKind.End);
    expect(fn).not.toHaveBeenCalled();

    await promise;
    expect(fn).toHaveBeenCalledWith(1);
  });

  it('creates a Promise for synchronous sources', async () => {
    const fn = vi.fn();
    await sinks.toPromise(sources.fromArray([1, 2, 3])).then(fn);
    expect(fn).toHaveBeenCalledWith(3);
  });
});

describe('toAsyncIterable', () => {
  it('creates an async iterable mirroring the Wonka source', async () => {
    let pulls = 0;
    let sink: Sink<any> | null = null;

    const source: Source<any> = _sink => {
      sink = _sink;
      sink(
        start(signal => {
          if (signal === TalkbackKind.Pull) pulls++;
        })
      );
    };

    const asyncIterator = sinks.toAsyncIterable(source)[Symbol.asyncIterator]();
    const next$ = asyncIterator.next();

    sink!(push(0));
    expect(await next$).toEqual({ value: 0, done: false });
    expect(pulls).toBe(1);

    sink!(push(1));
    expect(await asyncIterator.next()).toEqual({ value: 1, done: false });
    expect(pulls).toBe(2);

    sink!(SignalKind.End);
    expect(await asyncIterator.next()).toEqual({ done: true });
    expect(pulls).toBe(2);
  });

  it('buffers actively pushed values', async () => {
    let pulls = 0;
    let sink: Sink<any> | null = null;

    const source: Source<any> = _sink => {
      sink = _sink;
      sink(
        start(signal => {
          if (signal === TalkbackKind.Pull) pulls++;
        })
      );
    };

    const asyncIterator = sinks.toAsyncIterable(source)[Symbol.asyncIterator]();
    const next$ = asyncIterator.next();

    sink!(push(0));
    sink!(push(1));
    sink!(SignalKind.End);

    expect(pulls).toBe(1);
    expect(await next$).toEqual({ value: 0, done: false });
    expect(await asyncIterator.next()).toEqual({ value: 1, done: false });
    expect(await asyncIterator.next()).toEqual({ done: true });
  });

  it('asynchronously waits for pulled values', async () => {
    let pulls = 0;
    let sink: Sink<any> | null = null;

    const source: Source<any> = _sink => {
      sink = _sink;
      sink(
        start(signal => {
          if (signal === TalkbackKind.Pull) pulls++;
        })
      );
    };

    const asyncIterator = sinks.toAsyncIterable(source)[Symbol.asyncIterator]();
    asyncIterator.next();
    expect(pulls).toBe(1);

    let resolved = false;

    const promise = asyncIterator.next().then(value => {
      resolved = true;
      return value;
    });

    await Promise.resolve();
    expect(resolved).toBe(false);

    sink!(push(0));
    sink!(SignalKind.End);
    expect(await promise).toEqual({ value: 0, done: false });
    expect(await asyncIterator.next()).toEqual({ done: true });
  });

  it('supports cancellation via return', async () => {
    let ended = false;
    let sink: Sink<any> | null = null;

    const source: Source<any> = _sink => {
      sink = _sink;
      sink(
        start(signal => {
          if (signal === TalkbackKind.Close) ended = true;
        })
      );
    };

    const asyncIterator = sinks.toAsyncIterable(source)[Symbol.asyncIterator]();
    const next$ = asyncIterator.next();

    sink!(push(0));
    expect(await next$).toEqual({ value: 0, done: false });
    expect(await asyncIterator.return!()).toEqual({ done: true });

    sink!(push(1));
    expect(await asyncIterator.next()).toEqual({ done: true });

    expect(ended).toBeTruthy();
  });

  it('supports for-await-of', async () => {
    let pulls = 0;

    const source: Source<any> = sink => {
      sink(
        start(signal => {
          if (signal === TalkbackKind.Pull) {
            sink(pulls < 3 ? push(pulls++) : SignalKind.End);
          }
        })
      );
    };

    const iterable = sinks.toAsyncIterable(source);
    const values: any[] = [];
    for await (const value of iterable) {
      values.push(value);
    }

    expect(values).toEqual([0, 1, 2]);
  });

  it('supports for-await-of with early break', async () => {
    let pulls = 0;
    let closed = false;

    const source: Source<any> = sink => {
      sink(
        start(signal => {
          if (signal === TalkbackKind.Pull) {
            sink(pulls < 3 ? push(pulls++) : SignalKind.End);
          } else {
            closed = true;
          }
        })
      );
    };

    const iterable = sinks.toAsyncIterable(source);
    for await (const value of iterable) {
      expect(value).toBe(0);
      break;
    }

    expect(closed).toBe(true);
  });
});

describe('toObservable', () => {
  it('creates an Observable mirroring the Wonka source', () => {
    const next = vi.fn();
    const complete = vi.fn();
    let pulls = 0;
    let sink: Sink<any> | null = null;

    const source: Source<any> = _sink => {
      sink = _sink;
      sink(
        start(signal => {
          if (signal === TalkbackKind.Pull) pulls++;
        })
      );
    };

    Observable.from(observable.toObservable(source) as any).subscribe({
      next,
      complete,
    });

    expect(pulls).toBe(1);
    sink!(push(0));
    expect(next).toHaveBeenCalledWith(0);
    sink!(push(1));
    expect(next).toHaveBeenCalledWith(1);
    sink!(SignalKind.End);
    expect(complete).toHaveBeenCalled();
  });

  it('forwards cancellations from the Observable as a talkback', () => {
    let ending = 0;
    const source: Source<any> = sink =>
      sink(
        start(signal => {
          if (signal === TalkbackKind.Close) ending++;
        })
      );

    const sub = Observable.from(observable.toObservable(source) as any).subscribe({});

    expect(ending).toBe(0);
    sub.unsubscribe();
    expect(ending).toBe(1);
  });
});

describe('toCallbag', () => {
  it('creates a Callbag mirroring the Wonka source', () => {
    const fn = vi.fn();
    let pulls = 0;
    let sink: Sink<any> | null = null;

    const source: Source<any> = _sink => {
      sink = _sink;
      sink(
        start(signal => {
          if (signal === TalkbackKind.Pull) pulls++;
        })
      );
    };

    callbagIterate(fn)(callbag.toCallbag(source));

    expect(pulls).toBe(1);
    sink!(push(0));
    expect(fn).toHaveBeenCalledWith(0);
    sink!(push(1));
    expect(fn).toHaveBeenCalledWith(1);
    sink!(SignalKind.End);
  });

  it('forwards cancellations from the Callbag as a talkback', () => {
    let ending = 0;
    const fn = vi.fn();

    const source: Source<any> = sink =>
      sink(
        start(signal => {
          if (signal === TalkbackKind.Pull) {
            sink(push(0));
          } else {
            ending++;
          }
        })
      );

    callbagIterate(fn)(callbagTake(1)(callbag.toCallbag(source) as any));

    expect(fn.mock.calls).toEqual([[0]]);
    expect(ending).toBe(1);
  });
});
