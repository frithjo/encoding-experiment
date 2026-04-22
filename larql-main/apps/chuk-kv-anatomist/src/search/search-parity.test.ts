import { describe, expect, it, vi } from 'vitest';
import {
  mapContextMapWithQuery,
  mapPositions,
  type RawContextMapResult,
  type RawContextMapWithQueryResult,
} from './context-map-mappers';
import { checkLazarusError, normalizeNativeToolResult } from './tool-result';
import { NativeSearchAdapter } from './native-adapter';

const rawBase: RawContextMapResult = {
  prompt: 'hi',
  layer: 1,
  top_k: 5,
  positions: [
    {
      position: 0,
      token: 'hi',
      token_id: 1,
      predictions: [
        { token: 'a', token_id: 2, probability: 0.9 },
        { token: 'b', token_id: 3, probability: 0.1 },
      ],
      entropy: 0.2,
      specificity: 0.8,
      residual_norm: 1,
      token_residual_angle: 45,
    },
  ],
};

describe('context map mappers (MCP vs native parity)', () => {
  it('mapPositions matches shared raw fixture', () => {
    const mapped = mapPositions(rawBase);
    expect(mapped).toHaveLength(1);
    expect(mapped[0].tokenId).toBe(1);
    expect(mapped[0].topProbability).toBe(0.9);
    expect(mapped[0].residualNorm).toBe(1);
    expect(mapped[0].tokenResidualAngle).toBe(45);
  });

  it('mapContextMapWithQuery adds queryAttention from raw overlay fields', () => {
    const raw: RawContextMapWithQueryResult = {
      ...rawBase,
      query: 'q',
      positions: [
        {
          ...rawBase.positions[0],
          h5_attention: 0.1,
          h4_attention: 0.2,
          h2_attention: 0.3,
          copy_head_rank: 4,
        },
      ],
    };
    const mapped = mapContextMapWithQuery(raw);
    expect(mapped[0].queryAttention).toEqual({
      h5: 0.1,
      h4: 0.2,
      h2: 0.3,
      copyHeadRank: 4,
    });
  });
});

describe('normalizeNativeToolResult', () => {
  it('unwraps { result: payload }', () => {
    const inner = { foo: 1 };
    expect(normalizeNativeToolResult({ result: inner })).toBe(inner);
  });

  it('passes through bare payload', () => {
    const inner = { bar: 2 };
    expect(normalizeNativeToolResult(inner)).toBe(inner);
  });
});

describe('checkLazarusError', () => {
  it('throws on error_type', () => {
    expect(() => checkLazarusError({ error_type: 'X', message: 'bad' })).toThrow('X: bad');
  });
});

describe('NativeSearchAdapter', () => {
  it('POST /tools/call and maps context_map like MCP', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => rawBase,
    });
    vi.stubGlobal('fetch', fetchMock);

    const adapter = new NativeSearchAdapter('http://native.test');
    const entries = await adapter.fetchContextMap('hi', 1, 5);

    expect(fetchMock).toHaveBeenCalledWith(
      'http://native.test/tools/call',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          name: 'context_map',
          arguments: { prompt: 'hi', layer: 1, top_k: 5 },
        }),
      }),
    );
    expect(entries).toEqual(mapPositions(rawBase));

    vi.unstubAllGlobals();
  });
});
