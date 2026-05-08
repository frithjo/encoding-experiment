export interface Preset {
  name: string;
  prompt: string;
  description: string;
}

export const PRESETS: Preset[] = [
  {
    name: 'The 12 Bytes',
    prompt: 'The capital of France is',
    description: 'Core finding: 1 head, 1 direction, 12 bytes',
  },
  {
    name: 'Chemical Symbol',
    prompt: 'The chemical symbol for gold is',
    description: 'Factual retrieval: Au',
  },
  {
    name: 'Largest Planet',
    prompt: 'The largest planet is',
    description: 'Factual retrieval: Jupiter',
  },
  {
    name: 'Speed of Light',
    prompt: 'The speed of light is approximately',
    description: 'Numeric factual retrieval',
  },
  {
    name: 'Shakespeare',
    prompt: 'Shakespeare wrote',
    description: 'Ambiguous factual retrieval',
  },
  {
    name: 'Freezing Point',
    prompt: 'Water freezes at',
    description: 'Numeric with unit ambiguity (0/32)',
  },
  {
    name: 'Moon Landing',
    prompt: 'Neil Armstrong was the first',
    description: 'Factual completion',
  },
  {
    name: 'First President',
    prompt: 'The first president of the United States was',
    description: 'Historical factual retrieval',
  },
];
