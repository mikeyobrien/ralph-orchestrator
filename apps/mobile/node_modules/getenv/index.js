const url = require('url');

let fallbacksDisabled = false;
let throwError = true;

function _value(varName, fallback) {
  const value = process.env[varName];
  if (value === undefined) {
    if (fallback === undefined && !throwError) {
      return value;
    }
    if (fallback === undefined) {
      throw new Error(
        'GetEnv.Nonexistent: ' + varName + ' does not exist ' + 'and no fallback value provided.'
      );
    }
    if (fallbacksDisabled) {
      throw new Error(
        'GetEnv.DisabledFallbacks: ' +
          varName +
          ' relying on fallback ' +
          'when fallbacks have been disabled'
      );
    }
    return '' + fallback;
  }
  return value;
}

const convert = {
  string: function (value) {
    return '' + value;
  },
  int: function (value) {
    const isInt = value.match(/^-?\d+$/);
    if (!isInt) {
      throw new Error('GetEnv.NoInteger: ' + value + ' is not an integer.');
    }

    return +value;
  },
  float: function (value) {
    const isInfinity = +value === Infinity || +value === -Infinity;
    if (isInfinity) {
      throw new Error('GetEnv.Infinity: ' + value + ' is set to +/-Infinity.');
    }

    const isFloat = !(isNaN(value) || value === '');
    if (!isFloat) {
      throw new Error('GetEnv.NoFloat: ' + value + ' is not a number.');
    }

    return +value;
  },
  bool: function (value) {
    const isBool =
      (value || '').toLowerCase() === 'true' || (value || '').toLowerCase() === 'false';
    if (!isBool) {
      throw new Error('GetEnv.NoBoolean: ' + value + ' is not a boolean.');
    }

    return (value || '').toLowerCase() === 'true';
  },
  boolish: function (value) {
    try {
      return convert.bool(value);
    } catch (err) {
      const isBool = value === '1' || value === '0';
      if (!isBool) {
        throw new Error('GetEnv.NoBoolean: ' + value + ' is not a boolean.');
      }

      return value === '1';
    }
  },
  url: url.parse,
};

function converter(type) {
  return function (varName, fallback) {
    if (typeof varName == 'string') {
      // default
      const value = _value(varName, fallback);
      return convert[type](value);
    } else {
      // multibert!
      return getenv.multi(varName);
    }
  };
}

const getenv = converter('string');

Object.keys(convert).forEach(function (type) {
  getenv[type] = converter(type);
});

getenv.array = function array(varName, type, fallback, separator) {
  type = type || 'string';
  separator = separator || /\s*,\s*/;
  if (Object.keys(convert).indexOf(type) === -1) {
    throw new Error('GetEnv.ArrayUndefinedType: Unknown array type ' + type);
  }
  const value = _value(varName, fallback);
  return value.split(separator).map(convert[type]);
};

getenv.multi = function multi(spec) {
  const result = {};
  for (let key in spec) {
    const value = spec[key];
    if (Array.isArray(value)) {
      // default value & typecast
      switch (value.length) {
        case 1: // no default value
        case 2: // no type casting
          result[key] = getenv(value[0], value[1]); // dirty, when case 1: value[1] is undefined
          break;
        case 3: // with typecast
          result[key] = getenv[value[2]](value[0], value[1]);
          break;
        default:
          // wtf?
          throw 'getenv.multi(): invalid spec';
          break;
      }
    } else {
      // value or throw
      result[key] = getenv(value);
    }
  }
  return result;
};

getenv.disableFallbacks = function () {
  fallbacksDisabled = true;
};

getenv.enableFallbacks = function () {
  fallbacksDisabled = false;
};

getenv.disableErrors = function () {
  throwError = false;
};

getenv.enableErrors = function () {
  throwError = true;
};

module.exports = getenv;
