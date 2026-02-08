const net = require("net");

const DEFAULT_PORT_RANGE_START = 11000;

function testPortAsync(port, hostname) {
  return new Promise(function(fulfill, reject) {
    var server = net.createServer();
    server.listen({ port: port, host: hostname }, function(err) {
      server.once("close", function() {
        setTimeout(() => fulfill(true), 0);
      });
      server.close();
    });
    server.on("error", function(err) {
      setTimeout(() => fulfill(false), 0);
    });
  });
}

async function availableAsync(port, options = {}) {
  const hostnames =
    options.hostnames && options.hostnames.length ? options.hostnames : [null];
  for (const hostname of hostnames) {
    if (!(await testPortAsync(port, hostname))) {
      return false;
    }
  }
  return true;
}

function freePortRangeAsync(rangeSize, rangeStart, options = {}) {
  rangeSize = rangeSize || 1;
  return new Promise((fulfill, reject) => {
    var lowPort = rangeStart || DEFAULT_PORT_RANGE_START;
    var awaitables = [];
    for (var i = 0; i < rangeSize; i++) {
      awaitables.push(availableAsync(lowPort + i, options));
    }
    return Promise.all(awaitables).then(function(results) {
      var ports = [];
      for (var i = 0; i < results.length; i++) {
        if (!results[i]) {
          return freePortRangeAsync(
            rangeSize,
            lowPort + rangeSize,
            options
          ).then(fulfill, reject);
        }
        ports.push(lowPort + i);
      }
      fulfill(ports);
    });
  });
}

async function freePortAsync(rangeStart, options = {}) {
  const result = await freePortRangeAsync(1, rangeStart, options);
  return result[0];
}

module.exports = freePortAsync;

module.exports.availableAsync = availableAsync;
module.exports.rangeAsync = freePortRangeAsync;
