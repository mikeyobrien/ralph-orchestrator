// Jest setup file
// Global mocks and configuration

// Silence React warnings in tests
global.console = {
  ...console,
  // Uncomment to silence specific log levels
  // warn: jest.fn(),
  // error: jest.fn(),
};
