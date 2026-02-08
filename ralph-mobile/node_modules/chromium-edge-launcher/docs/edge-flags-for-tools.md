# Edge Flags for Tooling

Many tools maintain a list of runtime flags for Edge to configure the environment. This file
is an attempt to document all edge flags that are relevant to tools, automation, benchmarking, etc.

All use cases are different, so you'll have to choose which flags are most appropriate.

## Commonly unwanted browser features

* `--disable-client-side-phishing-detection`: Disables client-side phishing detection
* `--disable-component-extensions-with-background-pages`: Disable some built-in extensions that aren't affected by `--disable-extensions`
* `--disable-default-apps`: Disable installation of default apps
* `--disable-extensions`: Disable all edge extensions
* `--mute-audio`: Mute any audio
* `--no-default-browser-check`: Disable the default browser check, do not prompt to set it as such
* `--no-first-run`: Skip first run wizards
* `--use-fake-device-for-media-stream`: Use fake device for Media Stream to replace camera and microphone
* `--use-file-for-fake-video-capture=<path-to-file>`: Use file for fake video capture (.y4m or .mjpeg) Needs `--use-fake-device-for-media-stream`

## Performance & web platform behavior

* `--allow-running-insecure-content`
* `--autoplay-policy=user-gesture-required`: Don't render video
* `--disable-background-timer-throttling`: Disable timers being throttled in background pages/tabs
* `--disable-backgrounding-occluded-windows`
* `--disable-features=ScriptStreaming`: V8 script streaming
* `--disable-hang-monitor`
* `--disable-ipc-flooding-protection`: Some javascript functions can be used to flood the browser process with IPC. By default, protection is on to limit the number of IPC sent to 10 per second per frame. This flag disables it. https://crrev.com/604305
* `--disable-notifications`: Disables the Web Notification and the Push APIs.
* `--disable-popup-blocking`: Disable popup blocking.  `--block-new-web-contents` is the strict version of this.
* `--disable-prompt-on-repost`: Reloading a page that came from a POST normally prompts the user.
* `--disable-renderer-backgrounding`: This disables non-foreground tabs from getting a lower process priority This doesn't (on its own) affect timers or painting behavior. [karma-chromium-edge-launcher#123](https://github.com/karma-runner/karma-chromium-edge-launcher/issues/123)
* `--js-flags=--random-seed=1157259157`: Initialize V8's RNG with a fixed seed.

## Test & debugging flags

* `--disable-device-discovery-notifications`: Avoid messages like "New printer on your network"
* `--enable-automation`: Disable a few things considered not appropriate for automation. ([Original design doc](https://docs.google.com/a/google.com/document/d/1JYj9K61UyxIYavR8_HATYIglR9T_rDwAtLLsD3fbDQg/preview), though renamed [here](https://codereview.chromium.org/2564973002#msg24)) [codesearch](https://cs.chromium.org/search/?q=kEnableAutomation&type=cs). Note that some projects have chosen to **avoid** using this flag: [web-platform-tests/wpt/#6348](https://github.com/web-platform-tests/wpt/pull/6348)
  - disables bubble notification about running development/unpacked extensions
  - disables the password saving UI (which [covers](https://source.chromium.org/chromium/chromium/src/+/master:chrome/browser/password_manager/chrome_password_manager_client.cc;l=295-298;drc=00053fb4d880a925c890193b74a8ff35e1cef2a0) the usecase of the [removed](https://bugs.chromium.org/p/chromedriver/issues/detail?id=1015) `--disable-save-password-bubble` flag)
  - disables infobar animations
  - disables auto-reloading on network errors ([source](https://cs.chromium.org/chromium/src/chrome/renderer/net/net_error_helper_core.cc?l=917&rcl=6eaf0af71262eb876764c6237ee2fe021a3e7a18))
  - means the default browser check prompt isn't shown
  - avoids showing these 3 infobars: ShowBadFlagsPrompt, GoogleApiKeysInfoBarDelegate, ObsoleteSystemInfoBarDelegate
  - adds this infobar: ![image](https://user-images.githubusercontent.com/39191/30349667-92a7a086-97c8-11e7-86b2-1365e3d407e3.png)
* `--enable-logging=stderr`: Logging behavior slightly more appropriate for a server-type process.
* `--log-level=0`: 0 means INFO and higher.
* `--password-store=basic`: Avoid potential instability of using Gnome Keyring or KDE wallet. [chromium/linux/password_storage.md](https://chromium.googlesource.com/chromium/src/+/master/docs/linux/password_storage.md) https://crbug.com/571003
* `--remote-debugging-pipe`: more secure than using protocol over a websocket
* `--silent-debugger-extension-api`: Does not show an infobar when a Edge extension attaches to a page using `edge.debugger` page. Required to attach to extension background pages.
* `--test-type`: Basically the 2014 version of `--enable-automation`. [codesearch](https://cs.chromium.org/search/?q=kTestType%5Cb&type=cs)
  - It avoids creating application stubs in ~/Applications on mac.
  - It makes exit codes slightly more correct
  - windows navigation jumplists arent updated https://crbug.com/389375
  - doesn't start some edge StartPageService
  - disables initializing chromecast service
  - "Component extensions with background pages are not enabled during tests because they generate a lot of background behavior that can interfere."
  - when quitting the browser, it disables additional checks that may stop that quitting process. (like unsaved form modifications or unhandled profile notifications..)
* `--use-mock-keychain`: Use mock keychain on Mac to prevent blocking permissions dialogs. https://crbug.com/865247

## Background updates, networking, reporting

* `--disable-background-networking`: Disable various background network services, including extension updating,safe browsing service, upgrade detector, translate, UMA
* `--disable-breakpad`: Disable crashdump collection (reporting is already disabled in Chromium)
* `--disable-component-update`: Don't update the browser 'components' listed at edge://components/
* `--disable-domain-reliability`: Disables Domain Reliability Monitoring, which tracks whether the browser has difficulty contacting Google-owned sites and uploads reports to Google.
* `--disable-sync`: Disable syncing to a Google account
* `--enable-crash-reporter-for-testing`: Used for turning on Breakpad crash reporting in a debug environment where crash reporting is typically compiled but disabled.
* `--metrics-recording-only`: Disable reporting to UMA, but allows for collection

## Rendering & GPU

* `--deterministic-mode`: An experimental meta flag. This sets the below indented flags which put the browser into a mode where rendering (border radius, etc) is deterministic and begin frames should be issued over DevTools Protocol. [codesearch](https://source.chromium.org/chromium/chromium/src/+/master:headless/app/headless_shell.cc;drc=df45d1abbc20abc7670643adda6d9625eea55b4d)
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
* `--force-device-scale-factor=1`
* `--new-window`: Launches URL in new browser window.
* `--window-position=0,0`
* `--window-size=1600,1024`

## Process management

* `--disable-features=site-per-process`: Disables OOPIF. https://www.chromium.org/Home/chromium-security/site-isolation
* `--process-per-tab`: [Doesn't do anything](https://source.chromium.org/chromium/chromium/src/+/master:content/public/common/content_switches.cc;l=602-605;drc=2149a93144ce2030ab20863c2983b6c9d7bfd177). Use --single-process instead.
* `--single-process`: Runs the renderer and plugins in the same process as the browser.

## Headless

* `--disable-dev-shm-usage`: Often used in Lambda, Cloud Functions scenarios. ([pptr issue](https://github.com/GoogleChrome/puppeteer/issues/1834), [crbug](https://bugs.chromium.org/p/chromium/issues/detail?id=736452))
* `--no-sandbox`: [Sometimes used](https://github.com/puppeteer/puppeteer/blob/main/docs/troubleshooting.md#setting-up-chrome-linux-sandbox) with headless, though not recommended.
* `--disable-gpu`: Sometimes [used](https://bugs.chromium.org/p/chromium/issues/detail?id=737678) with headless.

# ~Removed~ flags

* `--disable-add-to-shelf`: [Removed June 2017](https://codereview.chromium.org/2944283002)
* `--disable-background-downloads`: [Removed Oct 2014](https://codereview.chromium.org/607843002).
* `--disable-browser-side-navigation` Disabled PlzNavigate.
* `--disable-datasaver-prompt`
* `--disable-desktop-notifications`
* `--disable-features=TranslateUI`: renamed `TranslateUI` to `Translate` in [Sept 2020](https://chromium-review.googlesource.com/c/chromium/src/+/2404484).
* `--disable-infobars`: [Removed April 2014](https://codereview.chromium.org/240193003)
* `--disable-save-password-bubble`: [Removed May 2016](https://codereview.chromium.org/1978563002)
* `--disable-translate`: [Removed April 2017](https://codereview.chromium.org/2819813002/) Used to disable built-in Google Translate service.
* `--ignore-autoplay-restrictions`: [Removed December 2017](https://chromium-review.googlesource.com/#/c/816855/) Can use `--autoplay-policy=no-user-gesture-required` instead.
* `--safebrowsing-disable-auto-update`: [Removed Nov 2017](https://bugs.chromium.org/p/chromium/issues/detail?id=74848#c26)

# Sources

* [chromium-edge-launcher's flags](https://github.com/cezaraugusto/chromium-edge-launcher/blob/master/src/flags.ts)
* [Chromedriver's flags](https://cs.chromium.org/chromium/src/chrome/test/chromedriver/chrome_launcher.cc?type=cs&q=f:chrome_launcher++kDesktopSwitches&sq=package:chromium)
* [Puppeteer's flags](https://github.com/puppeteer/puppeteer/blob/main/src/node/Launcher.ts)
* [WebpageTest's flags](https://github.com/WPO-Foundation/wptagent/blob/master/internal/chrome_desktop.py)
* [Catapult's flags](https://source.chromium.org/search?q=f:catapult%20f:desktop%20symbol:GetBrowserStartupArgs&ss=chromium%2Fchromium%2Fsrc)
* [Karma's flags](https://github.com/karma-runner/karma-chromium-edge-launcher/blob/master/index.js)

# All Edge flags

* [The canonical list of Chrome command-line switches on peter.sh](http://peter.sh/experiments/chromium-command-line-switches/) (maintained by the Chromium team)

FYI: (Probably) all flags are defined in files matching the pattern of [`*_switches.cc`](f:_switches\.cc).
## Feature Flags FYI

Chromium and Blink use feature flags to disable/enable many features at runtime. Chromium has [~400 features](https://source.chromium.org/search?q=%22const%20base::Feature%22%20f:%5C.cc&sq=&ss=chromium%2Fchromium%2Fsrc) that can be toggled with `--enable-features` / `--disable-features`. Indepdently, Blink has [many features](https://source.chromium.org/chromium/chromium/src/+/master:out/Debug/gen/third_party/blink/renderer/platform/runtime_enabled_features.cc;l=1559;drc=170473ad887b7990079f1f996b126548569c5902) that can be toggled [with commandline switches](https://chromium.googlesource.com/chromium/src/+/master/third_party/blink/renderer/platform/RuntimeEnabledFeatures.md#command_line-switches): `--enable-blink-features` / `--disable-blink-features`.

