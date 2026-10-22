const redact = value => String(value).replace(/solomondarker:\/\/[^\s'"<>]+/gi, '[redacted-invite]');

function sanitizeError(error) {
  // Retain the original context, excluding cause/assertion metadata that Node
  // prints separately from an Error's stack on an unhandled rejection.
  const sanitized = new Error(redact(error.message));
  sanitized.name = redact(error.name);
  sanitized.stack = redact(error.stack);
  return sanitized;
}

module.exports = {redact, sanitizeError};
