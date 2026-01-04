// Mock for @tanstack/react-query
const React = require('react');

const QueryClient = class {
  constructor(options = {}) {
    this.options = options;
  }
};

const QueryClientProvider = ({ children }) => children;

const useQuery = jest.fn(() => ({
  data: undefined,
  isLoading: true,
  error: null,
  refetch: jest.fn(),
}));

const useMutation = jest.fn(() => ({
  mutate: jest.fn(),
  mutateAsync: jest.fn(),
  isLoading: false,
  error: null,
}));

module.exports = {
  QueryClient,
  QueryClientProvider,
  useQuery,
  useMutation,
};
