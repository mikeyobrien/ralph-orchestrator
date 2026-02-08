# wonka

## 6.3.5

### Patch Changes

- Exclude `sourcesContent` from published sourcemaps
  Submitted by [@kitten](https://github.com/kitten) (See [`68e608f`](https://github.com/0no-co/wonka/commit/68e608f46244e82d41c952ecfa1d7f0096e168f6))

## 6.3.4

### Patch Changes

- Add missing `Symbol.observable` global declaration back to typings
  Submitted by [@kitten](https://github.com/kitten) (See [#168](https://github.com/0no-co/wonka/pull/168))

## 6.3.3

### Patch Changes

- Improve compatibility of `fromAsyncIterable` and `toAsyncIterable`. The `toAsyncIterable` will now output an object that's both an `AsyncIterator` and an `AsyncIterable`. Both helpers will now use a polyfill for `Symbol.asyncIterator` to improve compatibility with the Hermes engine and Babel transpilation
  Submitted by [@kitten](https://github.com/kitten) (See [#165](https://github.com/0no-co/wonka/pull/165))

## 6.3.2

### Patch Changes

- Publish with npm provenance
  Submitted by [@kitten](https://github.com/kitten) (See [#161](https://github.com/0no-co/wonka/pull/161))

## 6.3.1

### Patch Changes

- ⚠️ Fix missing `declare` keyword on internal ambient enums
  Submitted by [@kitten](https://github.com/kitten) (See [#159](https://github.com/0no-co/wonka/pull/159))

## 6.3.0

### Minor Changes

- Add `addOne` argument to `takeWhile`, allowing an additional value to be issued
  Submitted by [@kitten](https://github.com/kitten) (See [#156](https://github.com/0no-co/wonka/pull/156))

### Patch Changes

- Convert `Push<T>` and `Start<T>` signals to `{ tag, 0: value }` objects, which are sufficiently backwards compatible and result in slightly faster execution in v8
  Submitted by [@kitten](https://github.com/kitten) (See [#155](https://github.com/0no-co/wonka/pull/155))

## 6.2.6

### Patch Changes

- ⚠️ Fix missing source contents in Wonka sourcemaps
  Submitted by [@kitten](https://github.com/kitten) (See [`56d9708`](https://github.com/0no-co/wonka/commit/56d970861424fddd403262bf85d7e1e3572b15e2))
- ⚠️ Fix internal `SignalKind` and `TalkbackKind` enums not compiling away
  Submitted by [@kitten](https://github.com/kitten) (See [#154](https://github.com/0no-co/wonka/pull/154))

## 6.2.5

### Patch Changes

- Make `closed: boolean` on `ObservableSubscription`s a required field to comply with the Observable proposal's type spec
  Submitted by [@naporin0624](https://github.com/naporin0624) (See [#151](https://github.com/0no-co/wonka/pull/151))

## 6.2.4

### Patch Changes

- Add missing overload definition for `filter`, which allows types to be narrowed, e.g. by specifying a type predicate return type
  Submitted by [@kitten](https://github.com/kitten) (See [#149](https://github.com/0no-co/wonka/pull/149))

## 6.2.3

### Patch Changes

- ⚠️ Fix overload of `pipe` type not being applied in bundled `d.ts` file, by [@kitten](https://github.com/kitten) (See [#144](https://github.com/0no-co/wonka/pull/144))

## 6.2.2

### Patch Changes

- ⚠️ Fix missing `Symbol.observable` typings and remove `const enum` exports, which aren't usable in isolated modules, by [@kitten](https://github.com/kitten) (See [#141](https://github.com/0no-co/wonka/pull/141))

## 6.2.1

### Patch Changes

- ⚠️ Fix accidental addition of `postinstall` script rather than `prepare` script, by [@kitten](https://github.com/kitten) (See [#138](https://github.com/0no-co/wonka/pull/138))

## 6.2.0

### Minor Changes

- Implement `toAsyncIterable`, converting a Wonka source to a JS Async Iterable, by [@kitten](https://github.com/kitten) (See [#133](https://github.com/0no-co/wonka/pull/133))
- Implement `d.ts` bundling. Only a single `wonka.d.ts` typings file will now be available to TypeScript, by [@kitten](https://github.com/kitten) (See [#135](https://github.com/0no-co/wonka/pull/135))
- Add extensive TSDoc documentation for all `wonka` internals and exports. This will replace the documentation and give consumers more guidance on each of the library's extensive utilities, by [@kitten](https://github.com/kitten) (See [#136](https://github.com/0no-co/wonka/pull/136))

### Patch Changes

- ⚠️ Fix promise timing by adding missing `Promise.resolve()` tick to `toPromise` sink function, by [@kitten](https://github.com/kitten) (See [#131](https://github.com/0no-co/wonka/pull/131))
- ⚠️ Fix implementation of Observable spec as such that Observable.subscribe(onNext, onError, onComplete) becomes valid, by [@kitten](https://github.com/kitten) (See [#132](https://github.com/0no-co/wonka/pull/132))
