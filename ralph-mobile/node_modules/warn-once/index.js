const DEV = process.env.NODE_ENV !== "production";

const warnings = new Set();

function warnOnce(condition, ...rest) {
  if (DEV && condition) {
    const key = rest.join(" ");

    if (warnings.has(key)) {
      return;
    }

    warnings.add(key);
    console.warn(...rest);
  }
}

module.exports = warnOnce;
