var child_process = require('child_process');

function escapeArg(s) {
  if (s === '') {
    return "''";
  }
  if (/[^A-Za-z0-9_\/:=-]/.test(s)) {
    s = "'"+s.replace(/'/g,"'\\''")+"'";
    s = s.replace(/^(?:'')+/g, '') // unduplicate single-quote at the beginning
      .replace(/\\'''/g, "\\'" ); // remove non-escaped single-quote if there are enclosed between 2 escaped
  }
  return s;
}

function argsFromKeyVal(key, val, opts) {
  opts = opts || {};
  var prefix = '--';
  if (key.length === 1) {
    prefix = '-';
  }

  if ((val === true) || (val == null)) {
    return prefix + key;
  }  else if (val === false) {
    return [prefix + 'no' + key];
  } else if (Array.isArray(val)) {
    var a = [];
    for (var j = 0; j < val.length; j++) {
      a = a.concat(argsFromKeyVal(key, val[j], opts));
    }
    return a;
  } else {
    if ((key.length === 1) || opts.spaceForLongArgs) {
      return [prefix + key, val];
    } else {
      return [prefix + key + '=' + escapeArg(val)];
    }
  }

}

function argsListFromObject(args, opts) {
  if (!args) {
    return [];
  }
  if (Array.isArray(args)) {
    return argsListFromObject({_:args});
  }
  var a = [];
  var keys = Object.keys(args);
  for (var i = 0; i < keys.length; i++) {
    var key = keys[i];
    if (key[0] === '_') {
      continue;
    }
    var val = args[key];
    a = a.concat(argsFromKeyVal(key, val, opts));
  }

  if (args._) {
    for (var i = 0; i < args._.length; i++) {
      // Don't escape here since `execFile` will escape the args for us
      a.push('' + args._[i]);
    }
  }
  return a;
}

function execAsync(cmd, args, opts) {
  return new Promise(function (fulfill, reject) {
    child_process.execFile(cmd, argsListFromObject(args, opts), opts, function (err, result) {
      if (err) {
        reject(err);
      } else {
        fulfill(result);
      }
    });
  });
}

module.exports = execAsync;
module.exports.argsListFromObject = argsListFromObject;
module.exports.argsFromKeyVal = argsFromKeyVal;
module.exports.escapeArg = escapeArg;
