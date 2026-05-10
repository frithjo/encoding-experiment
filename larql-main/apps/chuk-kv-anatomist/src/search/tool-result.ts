/**
 * Normalize tool payloads from native HTTP (direct JSON) responses.
 */
export function assertToolPayload(parsed: unknown): asserts parsed is Record<string, unknown> {
  if (typeof parsed !== 'object' || parsed === null) {
    throw new Error('Invalid tool response: expected object');
  }
}

export function normalizeNativeToolResult(body: unknown): unknown {
  if (typeof body !== 'object' || body === null) {
    throw new Error('Invalid native response: expected object');
  }
  const o = body as Record<string, unknown>;
  if ('result' in o && o.result !== undefined) {
    return o.result;
  }
  return body;
}

export function checkLazarusError(parsed: Record<string, unknown>): void {
  if (parsed.error_type) {
    throw new Error(`${parsed.error_type}: ${parsed.message}`);
  }
}
