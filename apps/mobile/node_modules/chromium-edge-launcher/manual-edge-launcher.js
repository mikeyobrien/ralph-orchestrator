#!/usr/bin/env node

'use strict';
// Keep this file in sync with lighthouse
// TODO: have LH depend on this one.

/**
 * @fileoverview Script to launch a clean Edge instance on-demand.
 *
 * Assuming Lighthouse is installed globally or `npm link`ed, use via:
 *     edge-debug
 * Optionally enable extensions or pass a port, additional edge flags, and/or a URL
 *     edge-debug --port=9222
 *     edge-debug http://goat.com
 *     edge-debug --show-paint-rects
 *     edge-debug --enable-extensions
 */

require('./compiled-check.js')('./dist/chromium-edge-launcher.js');
const {Launcher, launch} = require('./dist/chromium-edge-launcher');

const args = process.argv.slice(2);
const edgeFlags = [];
let startingUrl;
let port;
let ignoreDefaultFlags;

if (args.length) {
  const providedFlags = args.filter(flag => flag.startsWith('--'));

  const portFlag = providedFlags.find(flag => flag.startsWith('--port='));
  if (portFlag) port = parseInt(portFlag.replace('--port=', ''), 10);

  const enableExtensions = !!providedFlags.find(flag => flag === '--enable-extensions');
  // The basic pattern for enabling Edge extensions
  if (enableExtensions) {
    ignoreDefaultFlags = true;
    edgeFlags.push(...Launcher.defaultFlags().filter(flag => flag !== '--disable-extensions'));
  }

  edgeFlags.push(...providedFlags);
  startingUrl = args.find(flag => !flag.startsWith('--'));
}

launch({
  startingUrl,
  port,
  ignoreDefaultFlags,
  edgeFlags,
})
// eslint-disable-next-line no-console
.then(v => console.log(`✨  Edge debugging port: ${v.port}`));
