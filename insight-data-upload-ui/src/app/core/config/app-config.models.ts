export interface AppConfig {
  application: {
    name: string;
    environment: string;
    defaultCaseDesignation: string;
    governmentWebsiteUrl: string;
  };
  springBoot: {
    baseUrl: string;
    paths: Record<string, string>;
  };
  features: Record<string, boolean>;
  virtualScroll: {
    rowHeightPx: number;
    bufferRows: number;
    renderedRows: number;
  };
  pagination: {
    defaultPageSize: number;
    pageSizeOptions: number[];
  };
  csvImport: {
    batchRows: number;
    readyRows: number;
    progressRows: number;
    sqliteCacheMb: number;
    maxFileBytes: number;
  };
  tooling: {
    devServerHost: string;
    devServerPort: number;
    devUrlHost: string;
  };
}
