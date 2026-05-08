/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_MCP_URL?: string;
  /** `mcp` (default) or `native` */
  readonly VITE_SEARCH_BACKEND?: string;
  /** Base URL for direct JSON `POST /tools/call` when backend is native */
  readonly VITE_NATIVE_SEARCH_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
