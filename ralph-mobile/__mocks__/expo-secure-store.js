// Mock for expo-secure-store
const store = {};

module.exports = {
  getItemAsync: jest.fn((key) => Promise.resolve(store[key] || null)),
  setItemAsync: jest.fn((key, value) => {
    store[key] = value;
    return Promise.resolve();
  }),
  deleteItemAsync: jest.fn((key) => {
    delete store[key];
    return Promise.resolve();
  }),
};
