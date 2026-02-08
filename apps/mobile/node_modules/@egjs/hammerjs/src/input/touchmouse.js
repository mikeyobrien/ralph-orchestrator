import Input from "../inputjs/input-constructor";
import TouchInput from "./touch";
import MouseInput from "./mouse";
import {
	INPUT_START,
	INPUT_END,
	INPUT_CANCEL,
	INPUT_TYPE_TOUCH,
	INPUT_TYPE_MOUSE,
} from "../inputjs/input-consts";

/**
 * @private
 * Combined touch and mouse input
 *
 * Touch has a higher priority then mouse, and while touching no mouse events are allowed.
 * This because touch devices also emit mouse events while doing a touch.
 *
 * @constructor
 * @extends Input
 */

const DEDUP_TIMEOUT = 2500;
const DEDUP_DISTANCE = 25;

function setLastTouch(eventData) {
	const { changedPointers: [touch] } = eventData;

	if (touch.identifier === this.primaryTouch) {
		const lastTouch = { x: touch.clientX, y: touch.clientY };
		const lts = this.lastTouches;

		this.lastTouches.push(lastTouch);


		const removeLastTouch = function() {
			const i = lts.indexOf(lastTouch);

			if (i > -1) {
				lts.splice(i, 1);
			}
		};

		setTimeout(removeLastTouch, DEDUP_TIMEOUT);
	}
}


function recordTouches(eventType, eventData) {
	if (eventType & INPUT_START) {
		this.primaryTouch = eventData.changedPointers[0].identifier;
		setLastTouch.call(this, eventData);
	} else if (eventType & (INPUT_END | INPUT_CANCEL)) {
		setLastTouch.call(this, eventData);
	}
}
function isSyntheticEvent(eventData) {
	const x = eventData.srcEvent.clientX;
	const y = eventData.srcEvent.clientY;

	for (let i = 0; i < this.lastTouches.length; i++) {
		const t = this.lastTouches[i];
		const dx = Math.abs(x - t.x);
		const dy = Math.abs(y - t.y);

		if (dx <= DEDUP_DISTANCE && dy <= DEDUP_DISTANCE) {
			return true;
		}
	}
	return false;
}


export default class TouchMouseInput extends Input {
	constructor(manager, callback) {
		super(manager, callback);

		this.touch = new TouchInput(this.manager, this.handler);
		this.mouse = new MouseInput(this.manager, this.handler);
		this.primaryTouch = null;
		this.lastTouches = [];
	}

	/**
	 * @private
	 * handle mouse and touch events
	 * @param {Hammer} manager
	 * @param {String} inputEvent
	 * @param {Object} inputData
	 */
	handler = (manager, inputEvent, inputData) => {
		const isTouch = (inputData.pointerType === INPUT_TYPE_TOUCH);
		const isMouse = (inputData.pointerType === INPUT_TYPE_MOUSE);

		if (isMouse && inputData.sourceCapabilities && inputData.sourceCapabilities.firesTouchEvents) {
			return;
		}

		// when we're in a touch event, record touches to  de-dupe synthetic mouse event
		if (isTouch) {
			recordTouches.call(this, inputEvent, inputData);
		} else if (isMouse && isSyntheticEvent.call(this, inputData)) {
			return;
		}

		this.callback(manager, inputEvent, inputData);
	}

	/**
	 * @private
	 * remove the event listeners
	 */
	destroy() {
		this.touch.destroy();
		this.mouse.destroy();
	}
}
