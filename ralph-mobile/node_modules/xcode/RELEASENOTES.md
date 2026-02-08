<!--
#
# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements.  See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership.  The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License.  You may obtain a copy of the License at
#
# http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
#  KIND, either express or implied.  See the License for the
# specific language governing permissions and limitations
# under the License.
#
-->
# Cordova-node-xcode Release Notes

### 3.0.1 (May 15, 2020)

* [GH-109](https://github.com/apache/cordova-node-xcode/pull/109) Add targetName to paramter to getBuildProperty

### 3.0.0 (Apr 09, 2020)

* node-xcode-3.0.0 Added NOTICE for release
* [GH-107](https://github.com/apache/cordova-node-xcode/pull/107) doc(README): fix CI badge url
* [GH-106](https://github.com/apache/cordova-node-xcode/pull/106) Update build property by `targetname`
* [GH-79](https://github.com/apache/cordova-node-xcode/pull/79) feat: add new optional paramter to `pbxProject.addTarget`
* [GH-96](https://github.com/apache/cordova-node-xcode/pull/96) ci: switch travis ci for github actions
* [GH-94](https://github.com/apache/cordova-node-xcode/pull/94) breaking(npm): bump dependencies
  * `simple-plist@^1.1.0`
  * `uuid@^7.0.3`
* [GH-93](https://github.com/apache/cordova-node-xcode/pull/93) chore(npm): use short notation for repo & bugs
* [GH-90](https://github.com/apache/cordova-node-xcode/pull/90) ci(travis): removes Node.js v6 and v8 from testing
* [GH-92](https://github.com/apache/cordova-node-xcode/pull/92) chore: bump node requirement (`>=10`)
* [GH-89](https://github.com/apache/cordova-node-xcode/pull/89) chore(npm): improve ignore list

### 2.1.0 (December 12, 2019)
* Add target test coverage ([#82](https://github.com/apache/cordova-node-xcode/pull/82))
* ensure coverage for addTargetDependency with invalid input ([#81](https://github.com/apache/cordova-node-xcode/pull/81))
* Support watch2 apps/extensions ([#56](https://github.com/apache/cordova-node-xcode/pull/56))
* Test coverage: `addTarget` add to main project as dependency ([#76](https://github.com/apache/cordova-node-xcode/pull/76))
* remove internal propReplace function not needed ([#69](https://github.com/apache/cordova-node-xcode/pull/69))
* Add proper `filetypeForProducttype` test coverage ([#72](https://github.com/apache/cordova-node-xcode/pull/72))
* Test existing WatchKit support ([#71](https://github.com/apache/cordova-node-xcode/pull/71))
* fix a comment in lib/pbxProject.js ([#68](https://github.com/apache/cordova-node-xcode/pull/68))
* Test coverage for `app_extension` target types ([#66](https://github.com/apache/cordova-node-xcode/pull/66))
* add dependencies for per-file testing ([#67](https://github.com/apache/cordova-node-xcode/pull/67))
* Add Node.js 12 to CI Services ([#51](https://github.com/apache/cordova-node-xcode/pull/51))
* Update writeObjectsSections to match current Xcode format ([#46](https://github.com/apache/cordova-node-xcode/pull/46))

### 2.0.0 (Jan 15, 2019)
* Updated to use ECMAScript 2015 Object.assign. ([#14](https://github.com/apache/cordova-node-xcode/pull/14))
* fix: simple-plist@1 update in dependencies ([#30](https://github.com/apache/cordova-node-xcode/pull/30))
* drop support for Node.js pre-6.0 [#29](https://github.com/apache/cordova-node-xcode/pull/29)

### 1.1.0 (Dec 19, 2018)
* feat: omit objects with empty values ([#24](https://github.com/apache/cordova-node-xcode/pull/24))
* Resolve issue with `pbxFile` extension ([#31](https://github.com/apache/cordova-node-xcode/pull/31))
* uuid@3 update ([#40](https://github.com/apache/cordova-node-xcode/pull/40))
* move `pegjs` to `devDependencies` in `package.json` ([#10](https://github.com/apache/cordova-node-xcode/pull/10))
* Fixed bug where comment is not removed on removing embedded frameworks. ([#5](https://github.com/apache/cordova-node-xcode/pull/5))
* Remove trailing whitespace from `lib/pbxWriter.js` ([#35](https://github.com/apache/cordova-node-xcode/pull/35))
* docs(readme): highlight code example ([#25](https://github.com/apache/cordova-node-xcode/pull/25))
* update invalid link to PEGjs in `README.md` ([#8](https://github.com/apache/cordova-node-xcode/pull/8))
* [CB-14145](https://issues.apache.org/jira/browse/CB-14145) explicit nodeunit@^0.11.3 update in `devDependencies` ([#10](https://github.com/apache/cordova-node-xcode/pull/10))

### 1.0.0 (Oct 4, 2017)
* Bump version to 1.0.0 to represent stability and follow semver more closely
* Fix null-access errors in `addTo/removeFrom*PbxGroup` methods
* Fix possible null-access error in `removeFromFrameworksPbxGroup`
* add check for `isArray` so that strings don't cause an error when calling `.filter`
* Updated License, Copyright, Contributors and repo url, in prep for contributing this project to Apache Cordova
