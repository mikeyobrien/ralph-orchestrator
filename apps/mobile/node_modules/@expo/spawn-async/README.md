# spawn-async [![Tests](https://github.com/expo/spawn-async/actions/workflows/main.yml/badge.svg)](https://github.com/expo/spawn-async/actions/workflows/main.yml)

A cross-platform version of Node's `child_process.spawn` as an async function that returns a promise. Supports Node 12 LTS and up.

## Usage:
```js
import spawnAsync from '@expo/spawn-async';

(async function () {
  let resultPromise = spawnAsync('echo', ['hello', 'world']);
  let spawnedChildProcess = resultPromise.child;
  try {
    let {
      pid,
      output: [stdout, stderr],
      stdout,
      stderr,
      status,
      signal,
    } = await resultPromise;
  } catch (e) {
    console.error(e.stack);
    // The error object also has the same properties as the result object
  }
})();
```

## API

`spawnAsync` takes the same arguments as [`child_process.spawn`](https://nodejs.org/api/child_process.html#child_process_child_process_spawn_command_args_options). Its options are the same as those of `child_process.spawn` plus:

- `ignoreStdio`: whether to ignore waiting for the child process's stdio streams to close before resolving the result promise. When ignoring stdio, the returned values for `stdout` and `stderr` will be empty strings. The default value of this option is `false`.

It returns a promise whose result is an object with these properties:

- `pid`: the process ID of the spawned child process
- `output`: an array with stdout and stderr's output
- `stdout`: a string of what the child process wrote to stdout
- `stderr`: a string of what the child process wrote to stderr
- `status`: the exit code of the child process
- `signal`: the signal (ex: `SIGTERM`) used to stop the child process if it did not exit on its own

If there's an error running the child process or it exits with a non-zero status code, `spawnAsync` rejects the returned promise. The Error object also has the properties listed above.

### Accessing the child process

Sometimes you may want to access the child process object--for example, if you wanted to attach event handlers to `stdio` or `stderr` and process data as it is available instead of waiting for the process to be resolved.

You can do this by accessing `.child` on the Promise that is returned by `spawnAsync`.

Here is an example:
```js
(async () => {
  let ffmpeg$ = spawnAsync('ffmpeg', ['-i', 'path/to/source.flac', '-codec:a', 'libmp3lame', '-b:a', '320k', '-ar', '44100', 'path/to/output.mp3']);
  let childProcess = ffmpeg$.child;
  childProcess.stdout.on('data', (data) => {
    console.log(`ffmpeg stdout: ${data}`);
  });
  childProcess.stderr.on('data', (data) => {
    console.error(`ffmpeg stderr: ${data}`);
  });
  let result = await ffmpeg$;
  console.log(`ffmpeg pid ${result.pid} exited with code ${result.code}`);
})();

```
