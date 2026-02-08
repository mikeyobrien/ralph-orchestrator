# [hammer.js][hammerjs-url]  [![NPM Version][npm-image]][npm-url]  [![NPM Downloads][downloads-image]][downloads-url]  [![Build Status][travis-image]][travis-url]
> A JavaScript library for detecting touch gestures.

## What is this repository?

It seems like that `hammer.js` repository is no more activated. 
  - [Is this library still actively maintained?](https://github.com/hammerjs/hammer.js/issues/1197)

But our library ([egjs](https://naver.github.io/egjs/)) has deep dependency to hammer.js for touch interaction.
And some pending issues were critical for us to make more light size, compatible with node.js , resolve module crash on umd envrioment.

So we solved following issues that are not resolved in original hammer.js

* [Support for hammer.js in node.js](https://github.com/naver/hammer.js/issues/3)
  - [Make hammerjs server friendly](https://github.com/hammerjs/hammer.js/issues/1060)
  - [Hammer.js fails in node since window is undefined](https://github.com/hammerjs/hammer.js/issues/930)
* [Support ES Module for Tree Shaking](https://github.com/naver/hammer.js/issues/12)
* [Change the priority of define and exports for umd](https://github.com/naver/hammer.js/issues/11)
* [Polymer/shadow-dom compatibility](https://github.com/naver/hammer.js/issues/21)

We would maintain this repository to fix remain bugs and enhance it until original repository activates.


## Installation
### NPM
```sh
npm install --save @egjs/hammerjs
```

**or**

### Yarn
```sh
yarn add @egjs/hammerjs
```

**or**

### CDN
[https://cdnjs.com/libraries/hammer.js/](https://cdnjs.com/libraries/hammer.js/)

> We did not provide CDN yet. leave the link to original hammerjs as is.

## Usage
hammer.js has a quick start option for gestures it already recognizes.
```js
// Get a reference to an element.
var square = document.querySelector('.square');

// Create an instance of Hammer with the reference.
var hammer = new Hammer(square);

// Subscribe to a quick start event: press, tap, or doubletap.
// For a full list of quick start events, read the documentation.
hammer.on('press', function(e) {
  e.target.classList.toggle('expand');
  console.log("You're pressing me!");
  console.log(e);
});
```

If you want to recognize your own gestures, such as `tripletap`, then you'll have to use these steps:
```js
// Get a reference to an element.
var square = document.querySelector('.square');

// Create a manager to manage the element.
var manager = new Hammer.Manager(square);

// Create a recognizer.
var TripleTap = new Hammer.Tap({
  event: 'tripletap',
  taps: 3
});

// Add the recognizer to the manager.
manager.add(TripleTap);

// Subscribe to the event.
manager.on('tripletap', function(e) {
  e.target.classList.toggle('expand');
  console.log("You're triple tapping me!");
  console.log(e);
});
```


## Examples
- [tap][tap]
- [double tap][double-tap]
- [press][press]
- [swipe][swipe]


## Documentation
For further information regarding hammer.js, please read our [documentation][hammerjs-url].


## Contributions  [![Github Issues][issues-image]][issues-url]  [![Github PRs][pulls-image]][pulls-url]  [![Slack][slack-image]][slack-url]
Feel encouraged to report issues or submit pull requests. When you're ready to do either, read our [contribution guidelines][contribution-guidelines]. If you're looking for another form of contribution, we love help answering questions on our [slack channel][slack-url].


## License
[MIT][license]

[hammerjs-url]: http://hammerjs.github.io
[hammerjs-url]: http://hammerjs.github.io

[npm-image]: https://img.shields.io/npm/v/@egjs/hammerjs.svg
[npm-url]: https://npmjs.org/package/@egjs/hammerjs

[travis-image]: https://img.shields.io/travis/stream-utils/raw-body/master.svg
[travis-url]: https://travis-ci.org/hammerjs/hammer.js

[downloads-image]: https://img.shields.io/npm/dm/@egjs/hammerjs.svg
[downloads-url]: https://npmjs.org/package/@egjs/hammerjs


<!-- Examples -->
[tap]: https://codepen.io/choskim/pen/WZggmg
[double-tap]: https://codepen.io/choskim/pen/vezzwZ
[press]: https://codepen.io/choskim/pen/RLYebL
[pan]: ''
[swipe]: https://codepen.io/choskim/pen/rGZqxa
[pinch]: ''
[rotate]: ''


<!-- Contributions -->
[issues-image]: https://img.shields.io/github/issues/hammerjs/hammer.js.svg
[issues-url]: https://github.com/hammerjs/hammer.js/issues

[pulls-image]: https://img.shields.io/github/issues-pr/hammerjs/hammer.js.svg
[pulls-url]: https://github.com/hammerjs/hammer.js/pulls

[slack-image]: https://hammerjs.herokuapp.com/badge.svg
[slack-url]: https://hammerjs.herokuapp.com/

[contribution-guidelines]: ./CONTRIBUTING.md

[license]: ./LICENSE.md
