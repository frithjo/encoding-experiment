import { createSearchClient } from './client';

/** Shared app instance; resolves MCP vs native from env at module load. */
export const searchClient = createSearchClient();
