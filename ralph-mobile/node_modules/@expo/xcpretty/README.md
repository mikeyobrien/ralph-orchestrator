# @expo/xcpretty

This can be used to parse and format xcodebuild logs.
The default error and warning format matches that of other tools in the Expo ecosystem.

```ts
import { Formatter } from '@expo/xcpretty';

const formatter = new Formatter({ projectRoot: '/' });

const lines = formatter.pipe('xcodebuild log results...');

for (const line of lines) {
  console.log(line);
}
```
