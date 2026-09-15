export const environment = {
  production: false,
  // The utility must call the BFF for domain operations; do not point the UI at 8081 directly.
  bffBaseUrl: 'http://localhost:8082',
  defaultCaseDesignation: 'Data Upload Utility'
};
