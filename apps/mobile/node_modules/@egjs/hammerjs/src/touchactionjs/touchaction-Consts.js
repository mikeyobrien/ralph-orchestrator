import getTouchActionProps from './get-touchaction-props';



// magical touchAction value
const TOUCH_ACTION_COMPUTE = 'compute';
const TOUCH_ACTION_AUTO = 'auto';
const TOUCH_ACTION_MANIPULATION = 'manipulation'; // not implemented
const TOUCH_ACTION_NONE = 'none';
const TOUCH_ACTION_PAN_X = 'pan-x';
const TOUCH_ACTION_PAN_Y = 'pan-y';
const TOUCH_ACTION_MAP = getTouchActionProps();

export {
  TOUCH_ACTION_AUTO,
  TOUCH_ACTION_COMPUTE,
  TOUCH_ACTION_MANIPULATION,
  TOUCH_ACTION_NONE,
  TOUCH_ACTION_PAN_X,
  TOUCH_ACTION_PAN_Y,
  TOUCH_ACTION_MAP
};
