import { describe, it, expect, beforeEach, vi } from 'vitest';

import { Source } from '../types';
import { fromValue, makeSubject } from '../sources';
import { forEach } from '../sinks';

import {
  passesPassivePull,
  passesActivePush,
  passesSinkClose,
  passesSourceEnd,
  passesSingleStart,
  passesStrictEnd,
} from './compliance';

import { combine, zip } from '../combine';

beforeEach(() => {
  vi.useFakeTimers();
});

describe('zip', () => {
  const noop = (source: Source<any>) => zip([fromValue(0), source]);

  passesPassivePull(noop, [0, 0]);
  passesActivePush(noop, [0, 0]);
  passesSinkClose(noop);
  passesSourceEnd(noop, [0, 0]);
  passesSingleStart(noop);
  passesStrictEnd(noop);

  it('emits the zipped values of two sources', () => {
    const { source: sourceA, next: nextA } = makeSubject<number>();
    const { source: sourceB, next: nextB } = makeSubject<number>();
    const fn = vi.fn();

    const combined = combine(sourceA, sourceB);
    forEach(fn)(combined);

    nextA(1);
    expect(fn).not.toHaveBeenCalled();
    nextB(2);
    expect(fn).toHaveBeenCalledWith([1, 2]);
  });

  it('emits the zipped values of three sources', () => {
    const { source: sourceA, next: nextA } = makeSubject<number>();
    const { source: sourceB, next: nextB } = makeSubject<number>();
    const { source: sourceC, next: nextC } = makeSubject<number>();
    const fn = vi.fn();

    const combined = zip([sourceA, sourceB, sourceC]);
    forEach(fn)(combined);

    nextA(1);
    expect(fn).not.toHaveBeenCalled();
    nextB(2);
    expect(fn).not.toHaveBeenCalled();
    nextC(3);
    expect(fn).toHaveBeenCalledWith([1, 2, 3]);
  });

  it('emits the zipped values of a dictionary of two sources', () => {
    const { source: sourceA, next: nextA } = makeSubject<number>();
    const { source: sourceB, next: nextB } = makeSubject<number>();
    const fn = vi.fn();

    const combined = zip({ a: sourceA, b: sourceB });
    forEach(fn)(combined);

    nextA(1);
    expect(fn).not.toHaveBeenCalled();
    nextB(2);
    expect(fn).toHaveBeenCalledWith({ a: 1, b: 2 });
  });
});
