# warn-once

Print a warning exactly once during development. Suitable for deprecation warnings, warnings for missing setup etc.

## Installation

Open a Terminal in the project root and run:

```sh
npm install warn-once
```

## Usage

The `warnOnce` function accepts a condition as the first argument. If the condition is `true`, then it'll print a warning:

```js
const warnOnce = require('warn-once');

// ...

warnOnce(someCondition, 'This is a warning message');
```

You can call `warnOnce` multiple times, but if the warning was printed already, it'll not be printed again.

The warning is only shown during development, i.e. if `NODE_ENV` is not set to `'production'`.
