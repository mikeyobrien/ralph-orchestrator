# resolve-workspace-root

Resolve the root of a workspace using [bun](https://bun.sh/guides/install/workspaces), [npm](https://docs.npmjs.com/cli/configuring-npm/package-json#workspaces), [pnpm](https://pnpm.io/workspaces), or [yarn](https://yarnpkg.com/features/workspaces).

- For bun, npm, and yarn - it looks for a parent **package.json** file, containing the `workspaces` config.
- For pnpm - it looks for a **package.json** and **pnpm-workspaces.yaml** file, containing the workspaces config.

## 🚀 How to use it

This package supports both synchronous and asynchronous lookups.

```ts
import { resolveWorkspaceRoot, resolveWorkspaceRootAsync } from 'resolve-workspace-root';

// Synchronous lookup, supporting bun, npm, pnpm, and yarn
const workspaceRoot = resolveWorkspaceRoot(__dirname);
// Synchronous lookup, supporting only bun, npm, and yarn
const workspaceRoot = resolveWorkspaceRoot(__dirname, { packageWorkspaces: false });
// Synchronous lookup, supporting only pnpm
const workspaceRoot = resolveWorkspaceRoot(__dirname, { pnpmWorkspaces: false });

// Asynchronous lookup, supporting bun, npm, pnpm, and yarn
const workspaceRoot = await resolveWorkspaceRootAsync(__dirname);
// Asynchronous lookup, supporting only bun, npm, and yarn
const workspaceRoot = await resolveWorkspaceRootAsync(__dirname, { packageWorkspaces: false });
// Asynchronous lookup, supporting only pnpm
const workspaceRoot = await resolveWorkspaceRootAsync(__dirname, { pnpmWorkspaces: false });

import { getWorkspaceGlobs, getWorkspaceGlobsAsync } from 'resolve-workspace-root';

// Synchronous lookup, supporting bun, npm, pnpm, and yarn
const workspaces = getWorkspaceGlobs(resolveWorkspaceRoot(__dirname));
// Asynchronous lookup, supporting bun, npm, pnpm, and yarn
const workspaces = await getWorkspaceGlobsAsync(resolveWorkspaceRoot(__dirname));
```

<div align="center">
  <br />
  with&nbsp;❤️&nbsp;&nbsp;<strong><a href="https://cedric.dev">Cedric</a></strong>
  <br />
</div>
