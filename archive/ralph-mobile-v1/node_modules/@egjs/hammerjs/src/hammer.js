import Manager from "./manager";
import defaults, { preset } from "./defaults";
import assign from './utils/assign';
import {
  INPUT_START,
  INPUT_MOVE,
  INPUT_END,
  INPUT_CANCEL,
  DIRECTION_NONE,
  DIRECTION_LEFT,
  DIRECTION_RIGHT,
  DIRECTION_UP,
  DIRECTION_DOWN,
  DIRECTION_HORIZONTAL,
  DIRECTION_VERTICAL,
  DIRECTION_ALL,
} from "./inputjs/input-consts";
import {
  STATE_POSSIBLE,
  STATE_BEGAN,
  STATE_CHANGED,
  STATE_ENDED,
  STATE_RECOGNIZED,
  STATE_CANCELLED,
  STATE_FAILED,
} from "./recognizerjs/recognizer-consts";

import Input from "./inputjs/input-constructor";
import TouchAction from "./touchactionjs/touchaction-constructor";
import TouchInput from "./input/touch";
import MouseInput from "./input/mouse";
import PointerEventInput from "./input/pointerevent";
import SingleTouchInput from "./input/singletouch";
import TouchMouseInput from "./input/touchmouse";

import Recognizer from "./recognizerjs/recognizer-constructor";
import AttrRecognizer from "./recognizers/attribute";
import TapRecognizer from "./recognizers/tap";
import PanRecognizer from "./recognizers/pan";
import SwipeRecognizer from "./recognizers/swipe";
import PinchRecognizer from "./recognizers/pinch";
import RotateRecognizer from "./recognizers/rotate";
import PressRecognizer from "./recognizers/press";

import addEventListeners from "./utils/add-event-listeners";
import removeEventListeners from "./utils/remove-event-listeners";
import each from "./utils/each";
import merge from "./utils/merge";
import extend from "./utils/extend";
import inherit from "./utils/inherit";
import bindFn from "./utils/bind-fn";
import prefixed from "./utils/prefixed";
import toArray from "./utils/to-array";
import uniqueArray from "./utils/unique-array";
import splitStr from "./utils/split-str";
import inArray from "./utils/in-array";
import boolOrFn from "./utils/bool-or-fn";
import hasParent from "./utils/has-parent";
/**
 * @private
 * Simple way to create a manager with a default set of recognizers.
 * @param {HTMLElement} element
 * @param {Object} [options]
 * @constructor
 */
export default class Hammer {
	/**
   * @private
   * @const {string}
   */
	static VERSION = "#__VERSION__#";
	static DIRECTION_ALL = DIRECTION_ALL;
	static DIRECTION_DOWN = DIRECTION_DOWN;
	static DIRECTION_LEFT = DIRECTION_LEFT;
	static DIRECTION_RIGHT = DIRECTION_RIGHT;
	static DIRECTION_UP = DIRECTION_UP;
	static DIRECTION_HORIZONTAL = DIRECTION_HORIZONTAL;
	static DIRECTION_VERTICAL = DIRECTION_VERTICAL;
	static DIRECTION_NONE = DIRECTION_NONE;
	static DIRECTION_DOWN = DIRECTION_DOWN;
	static INPUT_START = INPUT_START;
	static INPUT_MOVE = INPUT_MOVE;
  static INPUT_END = INPUT_END;
	static INPUT_CANCEL = INPUT_CANCEL;
	static STATE_POSSIBLE = STATE_POSSIBLE;
	static STATE_BEGAN = STATE_BEGAN;
	static STATE_CHANGED = STATE_CHANGED;
	static STATE_ENDED = STATE_ENDED;
	static STATE_RECOGNIZED = STATE_RECOGNIZED;
	static STATE_CANCELLED = STATE_CANCELLED;
	static STATE_FAILED = STATE_FAILED;
	static Manager = Manager;
	static Input = Input;
	static TouchAction = TouchAction;
	static TouchInput = TouchInput;
	static MouseInput = MouseInput;
	static PointerEventInput = PointerEventInput;
	static TouchMouseInput = TouchMouseInput;
	static SingleTouchInput = SingleTouchInput;
	static Recognizer = Recognizer;
	static AttrRecognizer = AttrRecognizer;
	static Tap = TapRecognizer;
	static Pan = PanRecognizer;
	static Swipe = SwipeRecognizer;
	static Pinch = PinchRecognizer;
	static Rotate = RotateRecognizer;
	static Press = PressRecognizer;
	static on = addEventListeners;
	static off = removeEventListeners;
	static each = each;
	static merge = merge;
	static extend = extend;
	static bindFn = bindFn;
	static assign = assign;
	static inherit = inherit;
	static bindFn = bindFn;
	static prefixed = prefixed;
	static toArray = toArray;
	static inArray = inArray;
	static uniqueArray = uniqueArray;
	static splitStr = splitStr;
	static boolOrFn = boolOrFn;
	static hasParent = hasParent;
	static addEventListeners = addEventListeners;
	static removeEventListeners = removeEventListeners;
	static defaults = assign({}, defaults, { preset });
	constructor(element, options = {}) {
		return new Manager(element, {
			recognizers: [
        // RecognizerClass, options, [recognizeWith, ...], [requireFailure, ...]
        ...preset
			],
			...options,
		});
	}
}
