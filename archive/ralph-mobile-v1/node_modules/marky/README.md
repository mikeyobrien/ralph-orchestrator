marky
======

JavaScript timer based on `performance.mark()` and `performance.measure()`, providing [high-resolution
timings](https://developer.mozilla.org/en-US/docs/Web/API/User_Timing_API) as well as nice Dev Tools visualizations.

For browsers that don't support `performance.mark()`, it falls back to 
`performance.now()` or `Date.now()`. In Node, it uses `process.hrtime()`.

Quick start
----

Install via npm:

    npm install marky

Or as a script tag:

```html
<script src="https://unpkg.com/marky/dist/marky.min.js"></script>
```

Then take some measurements:

```js
var marky = require('marky');

marky.mark('expensive operation');
doExpensiveOperation();
marky.stop('expensive operation');
```

Why?
---

The [User Timing API](https://developer.mozilla.org/en-US/docs/Web/API/User_Timing_API) is [more performant](https://gist.github.com/paulirish/2fad3834e2617fb199bc12e17058dde4)
than `console.time()` and `console.timeEnd()`,
and [more accurate](https://developer.mozilla.org/en-US/docs/Web/API/Performance/now) than `Date.now()`. Also, you get nice visualizations in Chrome Dev Tools:

![Chrome Dev Tools screenshot](doc/chrome.png)

As well as Edge F12 Tools:

![Edge F12 screenshot](doc/edge.png)

This is because `marky` adds standard
[PerformanceEntries](https://developer.mozilla.org/en-US/docs/Web/API/PerformanceEntry) to the [Performance Timeline](https://developer.mozilla.org/en-US/docs/Web/API/Performance_Timeline). Tools like [WebPageTest](http://blog.patrickmeenan.com/2013/07/measuring-performance-of-user-experience.html) and [Windows Performance Analyzer](https://blogs.windows.com/msedgedev/2016/05/11/top-down-analysis-wpt/) also surface them, and you can even [send them to your analytics provider](https://github.com/googlecodelabs/performance-analytics).

API
---

`marky.mark()` begins recording, and `marky.stop()` finishes recording:

```js
marky.mark('releaseTheHounds');
releaseTheHounds();
marky.stop('releaseTheHounds');
```

You can also do more complex scenarios:

```js
function setSail() {
  marky.mark('setSail');
  marky.mark('raiseTheAnchor');
  raiseTheAnchor();
  marky.stop('raiseTheAnchor');
  marky.mark('unfurlTheSails');
  unfurlTheSails();
  marky.stop('unfurlTheSails');
  marky.stop('setSail');
}
```

`marky.stop()` also returns a `PerformanceEntry`:

```js
marky.mark('manTheTorpedos');
manTheTorpedos();
var entry = marky.stop('manTheTorpedos');
```

The entry will look something like:

```json
{
  "entryType": "measure",
  "startTime": 1974112,
  "duration": 350,
  "name": "manTheTorpedos"
}
```

You can get all entries using:

```js
var entries = marky.getEntries();
```

This provides a list of all measures ordered by `startTime`, e.g.:

```json
[
  {
    "entryType": "measure",
    "startTime": 1974112,
    "duration": 350,
    "name": "numberOne"
  },
  {
    "entryType": "measure",
    "startTime": 1975108,
    "duration": 300,
    "name": "numberTwo"
  },
  {
    "entryType": "measure",
    "startTime": 1976127,
    "duration": 250,
    "name": "numberThree"
  }
]
```

You can also clear the entries using `marky.clear():`

```js
marky.clear()
```

After this, `marky.getEntries()` will return an empty list. (If the User Timing API is supported, this will delete all the `mark` and `measure` entries from the timeline.)

Browser support
----

`marky` has been tested in the following browsers/environments:

* IE 9+
* Safari 8+
* iOS 8+
* Android 4.4+
* Chrome
* Firefox
* Edge
* Node 4+

Per [the spec](https://www.w3.org/TR/resource-timing-1/#extensions-performance-interface), browsers only need to hold a minimum
of 150 entries in their Performance Timeline buffer. [In older versions of Firefox](https://bugzilla.mozilla.org/show_bug.cgi?id=1331135), the buffer
is throttled to 150, which for `marky`
means you can get a maximum of 50 entries from `marky.getEntries()` (because `marky` creates two marks and a measure).

If you need to get more than 50 entries from `marky.getEntries()`, you can do:

```js
if (typeof performance !== 'undefined' && performance.setResourceTimingBufferSize) {
  performance.setResourceTimingBufferSize(10000); // or however many you need
}
```

In Node and [browsers that don't support the User Timing API](http://caniuse.com/#feat=user-timing),
`marky` follows the behavior of Edge and Chrome, and does not limit the number of entries. `marky.stop()` and 
`marky.getEntries()` will return pseudo-`PerformanceEntry` objects.

See also
---

- [appmetrics.js](https://github.com/ebidel/appmetrics.js) – a library on top of `mark()`/`measure()` which reports to Google Analytics.

Credits
----

Thanks to [@toddreifsteck](https://github.com/toddreifsteck) for feedback on this project and clarifications on the User Timing API.
