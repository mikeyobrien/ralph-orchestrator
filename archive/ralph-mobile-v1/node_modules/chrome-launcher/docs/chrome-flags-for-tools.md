# Chrome Flags for Tooling

Many tools maintain a list of runtime flags for Chrome to configure the environment. This file
is an attempt to document all chrome flags that are relevant to tools, automation, benchmarking, etc.

All use cases are different, so you'll have to choose which flags are most appropriate.

Here's a **[Nov 2022 comparison of what flags](https://docs.google.com/spreadsheets/d/1n-vw_PCPS45jX3Jt9jQaAhFqBY6Ge1vWF_Pa0k7dCk4/edit#gid=1265672696)**  all these [tools](#sources) use.

## Commonly unwanted browser features

* `--disable-client-side-phishing-detection`: Disables client-side phishing detection
* `--disable-component-extensions-with-background-pages`: Disable some built-in extensions that aren't affected by `--disable-extensions`
* `--disable-default-apps`: Disable installation of default apps
* `--disable-extensions`: Disable all chrome extensions
* `--disable-features=InterestFeedContentSuggestions`: Disables the Discover feed on NTP
* `--disable-features=Translate`: Disables Chrome translation, both the manual option and the popup prompt when a page with differing language is detected.
* `--hide-scrollbars`: Hide scrollbars from screenshots.
* `--mute-audio`: Mute any audio
* `--no-default-browser-check`: Disable the default browser check, do not prompt to set it as such
* `--no-first-run`: Skip first run wizards

## Task throttling

* `--disable-background-timer-throttling`: Disable timers being throttled in background pages/tabs
* `--disable-backgrounding-occluded-windows`: Normally, Chrome will treat a 'foreground' tab instead as _backgrounded_ if the surrounding window is occluded (aka visually covered) by another window. This flag disables that.
* `--disable-features=CalculateNativeWinOcclusion`: Disable the feature of: Calculate window occlusion on Windows will be used in the future to throttle and potentially unload foreground tabs in occluded windows.
* `--disable-hang-monitor`: Suppresses hang monitor dialogs in renderer processes. This flag may allow slow unload handlers on a page to prevent the tab from closing.
* `--disable-ipc-flooding-protection`: Some javascript functions can be used to flood the browser process with IPC. By default, protection is on to limit the number of IPC sent to 10 per second per frame. This flag disables it. https://crrev.com/604305
* `--disable-renderer-backgrounding`: This disables non-foreground tabs from getting a lower process priority This doesn't (on its own) affect timers or painting behavior. [karma-chrome-launcher#123](https://github.com/karma-runner/karma-chrome-launcher/issues/123)

## Web platform behavior

* `--aggressive-cache-discard`
* `--allow-running-insecure-content`
* `--disable-back-forward-cache`: Disables the BackForwardCache feature.
* `--disable-features=AcceptCHFrame`: Disable accepting h2/h3 [ACCEPT_CH](https://datatracker.ietf.org/doc/html/draft-davidben-http-client-hint-reliability-02#section-4.3) Client Hints frames.
* `--disable-features=AutoExpandDetailsElement`: Removed in [Sept 2022](https://bugs.chromium.org/p/chromium/issues/detail?id=1185950#c62).
* `--disable-features=AvoidUnnecessaryBeforeUnloadCheckSync`: If enabled, this feature results in the browser process only asking the renderer process to run beforeunload handlers if it knows such handlers are registered. With `kAvoidUnnecessaryBeforeUnloadCheckSync`, content does not report a beforeunload handler is present. A ramification of this is navigations that would normally check beforeunload handlers before continuing will not, and navigation will synchronously continue.
* `--disable-features=BackForwardCache`: Disable the bfcache.
* `--disable-features=HeavyAdPrivacyMitigations`: Disables the privacy mitigations for the heavy ad intervention. This throttles the amount of interventions that can occur on a given host in a time period. It also adds noise to the thresholds used. This is separate from the intervention feature so it does not interfere with field trial activation, as this blocklist is created for every user, and noise is decided prior to seeing a heavy ad.
* `--disable-features=IsolateOrigins`
* `--disable-features=LazyFrameLoading`
* `--disable-features=ScriptStreaming`: V8 script streaming
* `--enable-precise-memory-info`: Make the values returned to window.performance.memory more granular and more up to date in shared worker. Without this flag, the memory information is still available, but it is bucketized and updated less frequently. This flag also applys to workers.
* `--js-flags=--random-seed=1157259157`: Initialize V8's RNG with a fixed seed.
* `--use-fake-device-for-media-stream`: Use fake device for Media Stream to replace camera and microphone
* `--use-fake-ui-for-media-stream`: Bypass the media stream infobar by selecting the default device for media streams (e.g. WebRTC). Works with --use-fake-device-for-media-stream.
* `--use-file-for-fake-video-capture=<path-to-file>`: Use file for fake video capture (.y4m or .mjpeg) Needs `--use-fake-device-for-media-stream`

## Interactivity suppression

* `--autoplay-policy=...`: Value of `user-gesture-required` to not autoplay video. Value of `no-user-gesture-required` to always autoplay video.
* `--deny-permission-prompts`: Suppress all permission prompts by automatically denying them.
* `--disable-external-intent-requests`: Disallow opening links in external applications
* `--disable-features=GlobalMediaControls`: Hide toolbar button that opens dialog for controlling media sessions.
* `--disable-features=ImprovedCookieControls`: Disables an improved UI for third-party cookie blocking in incognito mode.
* `--disable-notifications`: Disables the Web Notification and the Push APIs.
* `--disable-popup-blocking`: Disable popup blocking.  `--block-new-web-contents` is the strict version of this.
* `--disable-prompt-on-repost`: Reloading a page that came from a POST normally prompts the user.
* `--noerrdialogs`: Suppresses all error dialogs when present.

## Catch-all automation
* `--enable-automation`: Disable a few things considered not appropriate for automation. ([Original design doc](https://docs.google.com/a/google.com/document/d/1JYj9K61UyxIYavR8_HATYIglR9T_rDwAtLLsD3fbDQg/preview), though renamed [here](https://codereview.chromium.org/2564973002#msg24)) [codesearch](https://cs.chromium.org/search/?q=kEnableAutomation&type=cs). Note that some projects have chosen to **avoid** using this flag: [web-platform-tests/wpt/#6348](https://github.com/web-platform-tests/wpt/pull/6348), [crbug.com/1277272](https://crbug.com/1277272)
  - sets `window.navigator.webdriver` to `true` within all JS contexts. This is also set [when using](https://source.chromium.org/chromium/chromium/src/+/main:content/child/runtime_features.cc;l=374-376;drc=4a843634b8b3006e431add55968f6f45ee54d35e) `--headless`, `--remote-debugging-pipe` and `--remote-debugging-port=0` (yes, [_specifically_ 0](https://source.chromium.org/chromium/chromium/src/+/main:content/child/runtime_features.cc;l=412-427;drc=4a843634b8b3006e431add55968f6f45ee54d35e)).
  - disables bubble notification about running development/unpacked extensions ([source](https://source.chromium.org/chromium/chromium/src/+/main:chrome/browser/ui/extensions/extension_message_bubble_factory.cc;l=71-76;drc=1e6c1a39cbbc1dcad6e7828661d74d76463465ed))
  - disables the password saving UI (which [covers](https://source.chromium.org/chromium/chromium/src/+/main:chrome/browser/password_manager/chrome_password_manager_client.cc;l=301-308;drc=1e6c1a39cbbc1dcad6e7828661d74d76463465ed) the usecase of the [defunct](https://bugs.chromium.org/p/chromedriver/issues/detail?id=1015) `--disable-save-password-bubble` flag)
  - disables infobar animations ([source](https://source.chromium.org/chromium/chromium/src/+/main:components/infobars/content/content_infobar_manager.cc;l=48-52;drc=1e6c1a39cbbc1dcad6e7828661d74d76463465ed))
  - disables auto-reloading on network errors ([source](https://source.chromium.org/chromium/chromium/src/+/main:chrome/browser/chrome_content_browser_client.cc;l=1328-1331;drc=1e6c1a39cbbc1dcad6e7828661d74d76463465ed))
  - enables the CDP method [`Browser.getBrowserCommandLine`](https://chromedevtools.github.io/devtools-protocol/tot/Browser/#method-getBrowserCommandLine).
  - avoids showing these 4 infobars: ShowBadFlagsPrompt, GoogleApiKeysInfoBarDelegate, ObsoleteSystemInfoBarDelegate, LacrosButterBar
  - adds this infobar: ![image](https://user-images.githubusercontent.com/39191/30349667-92a7a086-97c8-11e7-86b2-1365e3d407e3.png) ... which is known to [adversely affect screenshots](https://bugs.chromium.org/p/chromium/issues/detail?id=1277272).
* `--test-type`: Basically the 2014 version of `--enable-automation`. [codesearch](https://cs.chromium.org/search/?q=kTestType%5Cb&type=cs)
  - It avoids creating application stubs in ~/Applications on mac.
  - It makes exit codes slightly more correct
  - windows navigation jumplists arent updated https://crbug.com/389375
  - doesn't start some chrome StartPageService
  - disables initializing chromecast service
  - "Component extensions with background pages are not enabled during tests because they generate a lot of background behavior that can interfere."
  - when quitting the browser, it disables additional checks that may stop that quitting process. (like unsaved form modifications or unhandled profile notifications..)
* `--remote-debugging-pipe`: more secure than using protocol over a websocket
* `--remote-debugging-port=...`: With a value of 0, Chrome will automatically select a useable port _and_ will set `navigator.webdriver` to `true`.
* `--silent-debugger-extension-api`: Does not show an infobar when a Chrome extension attaches to a page using `chrome.debugger` page. Required to attach to extension background pages.

## General

* `--enable-logging=stderr`: Logging behavior slightly more appropriate for a server-type process.
* `--log-level=0`: 0 means INFO and higher. `2` is the most verbose. Protip: Use `--enable-logging=stderr --v=2` and you may spot additional components active that you may want to disable.
* `--user-data-dir=...`: Directory where the browser stores the user profile.

## Chromium Annoyances

* `--disable-features=DialMediaRouteProvider`: Avoid the startup dialog for _Do you want the application “Chromium.app” to accept incoming network connections?_. This is a sub-component of the MediaRouter.
* `--password-store=basic`: Avoid potential instability of using Gnome Keyring or KDE wallet. [chromium/linux/password_storage.md](https://chromium.googlesource.com/chromium/src/+/main/docs/linux/password_storage.md) https://crbug.com/571003
* `--use-mock-keychain`: Use mock keychain on Mac to prevent the blocking permissions dialog abou: _Chrome wants to use your confidential information stored in your keychain_

## Background networking

* `--disable-background-networking`: Disable various background network services, including extension updating,safe browsing service, upgrade detector, translate, UMA
* `--disable-breakpad`: Disable crashdump collection (reporting is already disabled in Chromium)
* `--disable-component-update`: Don't update the browser 'components' listed at chrome://components/
* `--disable-domain-reliability`: Disables Domain Reliability Monitoring, which tracks whether the browser has difficulty contacting Google-owned sites and uploads reports to Google.
* `--disable-features=AutofillServerCommunication`:  Disables (mostly for hermetic testing) autofill server communication. The URL of the autofill server can further be controlled via the autofill-server-url param. The given URL should specify the complete autofill server API url up to the parent "directory" of the "query" and "upload" resources. i.e., https://other.autofill.server:port/tbproxy/af/
* `--disable-features=CertificateTransparencyComponentUpdater`
* `--disable-sync`: Disable syncing to a Google account
* `--enable-crash-reporter-for-testing`: Used for turning on Breakpad crash reporting in a debug environment where crash reporting is typically compiled but disabled.
* `--metrics-recording-only`: Disable reporting to UMA, but allows for collection
* `--disable-features=OptimizationHints`: Disable the [Chrome Optimization Guide](https://chromium.googlesource.com/chromium/src/+/HEAD/components/optimization_guide/) and networking with its service API
* `--disable-features=MediaRouter`: Disable the [Chrome Media Router](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/media/media_router.md) which creates some background network activity to discover castable targets.
* `--no-pings`: Don't send hyperlink auditing pings

## Rendering & GPU

* `--allow-pre-commit-input`: Allows processing of input before a frame has been committed. Used by headless. https://crbug.com/987626
* `--deterministic-mode`: An experimental meta flag. This sets the below indented flags which put the browser into a mode where rendering (border radius, etc) is deterministic and begin frames should be issued over DevTools Protocol. [codesearch](https://source.chromium.org/chromium/chromium/src/+/main:headless/app/headless_shell.cc;drc=df45d1abbc20abc7670643adda6d9625eea55b4d)
  - `--run-all-compositor-stages-before-draw`
  - `--disable-new-content-rendering-timeout`
  - `--enable-begin-frame-control`
  - `--disable-threaded-animation`
  - `--disable-threaded-scrolling`
  - `--disable-checker-imaging`
  - `--disable-image-animation-resync`
* `--disable-features=PaintHolding`: Don't defer paint commits (normally used to avoid flash of unstyled content)
* `--disable-partial-raster`: https://crbug.com/919955
* `--disable-skia-runtime-opts`: Do not use runtime-detected high-end CPU optimizations in Skia.
* `--in-process-gpu`: Saves some memory by moving GPU process into a browser process thread
* `--use-gl="swiftshader"`: Select which implementation of GL the GPU process should use. Options are: `desktop`: whatever desktop OpenGL the user has installed (Linux and Mac default). `egl`: whatever EGL / GLES2 the user has installed (Windows default - actually ANGLE). `swiftshader`: The SwiftShader software renderer.

## Window & screen management

* `--block-new-web-contents`: All pop-ups and calls to window.open will fail.
* `--force-color-profile=srgb`: Force all monitors to be treated as though they have the specified color profile.
* `--new-window`: Launches URL in new browser window.
* `--window-position=0,0`: Specify the initial window position: --window-position=x,y
* `--window-size=1600,1024`: Sets the initial window size. Provided as string in the format "800,600".

## Process management

* `--disable-features=DestroyProfileOnBrowserClose`:  Disable the feature of: Destroy profiles when their last browser window is closed, instead of when the browser exits.
* `--disable-features=site-per-process`: Disables OOPIF. https://www.chromium.org/Home/chromium-security/site-isolation
* `--no-service-autorun`: Disables the service process from adding itself as an autorun process. This does not delete existing autorun registrations, it just prevents the service from registering a new one.
* `--process-per-tab`: [Doesn't do anything](https://source.chromium.org/chromium/chromium/src/+/main:content/public/common/content_switches.cc;l=602-605;drc=2149a93144ce2030ab20863c2983b6c9d7bfd177). Use --single-process instead.
* `--single-process`: Runs the renderer and plugins in the same process as the browser.

## Headless

* `--headless`: Run in headless mode, i.e., without a UI or display server dependencies.
* `--headless=new`: New, native Headless mode. ([previously](https://bugs.chromium.org/p/chromium/issues/detail?id=1380881), `--headless=chrome`)
* `--no-sandbox`: [Sometimes used](https://github.com/puppeteer/puppeteer/blob/main/docs/troubleshooting.md#setting-up-chrome-linux-sandbox) with headless, though not recommended.
* `--disable-dev-shm-usage`: Often used in Lambda, Cloud Functions scenarios. ([pptr issue](https://github.com/GoogleChrome/puppeteer/issues/1834), [crbug](https://bugs.chromium.org/p/chromium/issues/detail?id=736452))
* `--disable-gpu`: Was often [used](https://bugs.chromium.org/p/chromium/issues/detail?id=737678) along with `--headless`, but as of 2021, isn't needed.

# ~Removed~ flags

* `--disable-add-to-shelf`: [Removed June 2017](https://codereview.chromium.org/2944283002)
* `--disable-background-downloads`: [Removed Oct 2014](https://codereview.chromium.org/607843002).
* `--disable-browser-side-navigation`: Removed. It disabled PlzNavigate.
* `--disable-datasaver-prompt`: Removed
* `--disable-desktop-notifications`: Removed
* `--disable-device-discovery-notifications`: Removed. Avoided messages like "New printer on your network". [Replaced](https://crbug.com/1020447#c1) with `--disable-features=MediaRouter`.
* `--disable-features=TranslateUI`: Removed as `TranslateUI` changed to `Translate` in [Sept 2020](https://chromium-review.googlesource.com/c/chromium/src/+/2404484).
* `--disable-infobars`: [Removed April 2014](https://codereview.chromium.org/240193003)
* `--disable-save-password-bubble`: [Removed May 2016](https://codereview.chromium.org/1978563002)
* `--disable-search-geolocation-disclosure`: Removed.
* `--disable-translate`: [Removed April 2017](https://codereview.chromium.org/2819813002/) Used to disable built-in Google Translate service.
* `--ignore-autoplay-restrictions`: [Removed December 2017](https://chromium-review.googlesource.com/#/c/816855/) Can use `--autoplay-policy=no-user-gesture-required` instead.
* `--safebrowsing-disable-auto-update`: [Removed Nov 2017](https://bugs.chromium.org/p/chromium/issues/detail?id=74848#c26)

# Sources

* [chrome-launcher's flags](https://github.com/GoogleChrome/chrome-launcher/blob/main/src/flags.ts)
* [Chromedriver's flags](https://cs.chromium.org/chromium/src/chrome/test/chromedriver/chrome_launcher.cc?type=cs&q=f:chrome_launcher++kDesktopSwitches&sq=package:chromium)
* [Puppeteer's flags](https://github.com/puppeteer/puppeteer/blob/3f2c0590f154aefd2ad3449a3f943ee79d1e33a9/packages/puppeteer-core/src/node/ChromeLauncher.ts#L159)
* [WebpageTest's flags](https://github.com/WPO-Foundation/wptagent/blob/master/internal/chrome_desktop.py)
* [Catapult's flags](https://source.chromium.org/chromium/chromium/src/+/main:third_party/catapult/telemetry/telemetry/internal/backends/chrome/chrome_startup_args.py) and [here](https://source.chromium.org/chromium/chromium/src/+/main:third_party/catapult/telemetry/telemetry/internal/backends/chrome/desktop_browser_finder.py;l=218;drc=4a0e6f034e9756605cfc837c8526588d6c13436b)
* [Karma's flags](https://github.com/karma-runner/karma-chrome-launcher/blob/master/index.js)


[The canonical list of Chrome command-line switches on peter.sh](http://peter.sh/experiments/chromium-command-line-switches/) (maintained by the Chromium team)

FYI: (Probably) all flags are defined in files matching the pattern of [`*_switches.cc`](https://source.chromium.org/search?q=f:_switches%5C.cc&ss=chromium%2Fchromium%2Fsrc).

## Feature Flags FYI

Chromium and Blink use feature flags to disable/enable many features at runtime. Chromium has [~400 features](https://source.chromium.org/search?q=%22const%20base::Feature%22%20f:%5C.cc&sq=&ss=chromium%2Fchromium%2Fsrc) that can be toggled with `--enable-features` / `--disable-features`. https://niek.github.io/chrome-features/ presents all of them very clearly.


Independently, Blink has [many features](https://source.chromium.org/chromium/chromium/src/+/main:out/Debug/gen/third_party/blink/renderer/platform/runtime_enabled_features.cc;l=1969;drc=d6e91a65ded605d8577f0651b3665c8206ae6ce3) that can be toggled [with commandline switches](https://chromium.googlesource.com/chromium/src/+/main/third_party/blink/renderer/platform/RuntimeEnabledFeatures.md#command_line-switches): `--enable-blink-features` / `--disable-blink-features`.
