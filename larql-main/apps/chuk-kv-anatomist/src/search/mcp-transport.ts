/**
 * MCP Streamable HTTP transport (SSE + session) for tools/call.
 */

interface MCPToolResult {
  content: { type: string; text: string }[];
}

export function createMcpTransport(mcpUrl: string) {
  let sessionId: string | null = null;
  let initPromise: Promise<void> | null = null;
  let msgCounter = 0;

  function nextId(): number {
    return ++msgCounter;
  }

  async function parseSSE(res: Response): Promise<Record<string, unknown>> {
    const text = await res.text();
    for (const block of text.split('\n\n')) {
      for (const line of block.split('\n')) {
        if (line.startsWith('data: ')) {
          const json = JSON.parse(line.slice(6));
          return json;
        }
      }
    }
    throw new Error('No data in SSE response');
  }

  async function initSession(): Promise<void> {
    const res = await fetch(`${mcpUrl}/mcp`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Accept: 'application/json, text/event-stream',
      },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: nextId(),
        method: 'initialize',
        params: {
          protocolVersion: '2025-03-26',
          capabilities: {},
          clientInfo: { name: 'kv-anatomist', version: '0.1.0' },
        },
      }),
    });

    if (!res.ok) throw new Error(`MCP init failed: ${res.status}`);

    const sid = res.headers.get('mcp-session-id');
    if (sid) sessionId = sid;

    await parseSSE(res);

    await fetch(`${mcpUrl}/mcp`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Accept: 'application/json, text/event-stream',
        ...(sessionId ? { 'mcp-session-id': sessionId } : {}),
      },
      body: JSON.stringify({
        jsonrpc: '2.0',
        method: 'notifications/initialized',
      }),
    });
  }

  async function ensureSession(): Promise<void> {
    if (sessionId) return;
    if (!initPromise) {
      initPromise = initSession().catch(err => {
        initPromise = null;
        throw err;
      });
    }
    await initPromise;
  }

  async function callTool(name: string, args: Record<string, unknown>): Promise<unknown> {
    await ensureSession();

    const res = await fetch(`${mcpUrl}/mcp`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Accept: 'application/json, text/event-stream',
        ...(sessionId ? { 'mcp-session-id': sessionId } : {}),
      },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: nextId(),
        method: 'tools/call',
        params: { name, arguments: args },
      }),
    });

    if (!res.ok) throw new Error(`MCP call failed: ${res.status}`);

    const contentType = res.headers.get('content-type') || '';
    let json: Record<string, unknown>;

    if (contentType.includes('text/event-stream')) {
      json = await parseSSE(res);
    } else {
      json = await res.json();
    }

    if (json.error) {
      const err = json.error as { message: string };
      throw new Error(err.message);
    }

    const result = json.result as MCPToolResult;
    const text = result?.content?.[0]?.text;
    if (!text) throw new Error('Empty response from MCP server');
    const parsed = JSON.parse(text) as Record<string, unknown>;
    if (parsed.error_type) {
      throw new Error(`${parsed.error_type}: ${parsed.message}`);
    }
    return parsed;
  }

  return { callTool };
}
