# chuk-kv-anatomist

**DEPRECATED:** This React UI is being migrated to Leptos. See [docs/leptos-migration-guide.md](../../docs/leptos-migration-guide.md) for migration status and details. See [docs/ui/README.md](../../docs/ui/README.md) for overall UI strategy.

Vite + React UI for inspecting KV / context maps against a **Lazarus** backend.

## Terminal-first workflow

The search contract is meant to be **configured and validated from the shell**: set env vars in your terminal, start the MCP or native backend from the terminal, and use the same tool names/JSON as the UI. The web app only consumes that contract at build time (`VITE_*`).

- **Smoke-test native tools from the terminal** (same body as `NativeSearchAdapter`):

```bash
export NATIVE_SEARCH_URL=http://127.0.0.1:8766   # or VITE_NATIVE_SEARCH_URL
./scripts/tools-call-example.sh get_model_info '{}'
./scripts/tools-call-example.sh context_map '{"prompt":"The capital of France is","layer":29,"top_k":5}'
```

- **Run the UI** after exporting `VITE_SEARCH_BACKEND`, `VITE_MCP_URL`, and/or `VITE_NATIVE_SEARCH_URL` in that same shell so Vite picks them up:

```bash
export VITE_SEARCH_BACKEND=mcp   # or native
npm run dev
```

## Search backends

| Variable | Purpose |
|----------|---------|
| `VITE_SEARCH_BACKEND` | `mcp` (default) or `native` |
| `VITE_MCP_URL` | MCP HTTP base (default `http://localhost:8765`) |
| `VITE_NATIVE_SEARCH_URL` | Required when `native`: base URL for **direct** `POST {base}/tools/call` with body `{ name, arguments }` returning the same JSON tool payload as MCP (optionally wrapped as `{ result: ... }`) |

Native mode avoids MCP session + SSE; implement `/tools/call` on your side to dispatch the same tool names (`context_map`, `get_model_info`, etc.).

---

# React + TypeScript + Vite

This template provides a minimal setup to get React working in Vite with HMR and some ESLint rules.

Currently, two official plugins are available:

- [@vitejs/plugin-react](https://github.com/vitejs/vite-plugin-react/blob/main/packages/plugin-react/README.md) uses [Babel](https://babeljs.io/) for Fast Refresh
- [@vitejs/plugin-react-swc](https://github.com/vitejs/vite-plugin-react-swc) uses [SWC](https://swc.rs/) for Fast Refresh

## Expanding the ESLint configuration

If you are developing a production application, we recommend updating the configuration to enable type aware lint rules:

- Configure the top-level `parserOptions` property like this:

```js
export default tseslint.config({
  languageOptions: {
    // other options...
    parserOptions: {
      project: ['./tsconfig.node.json', './tsconfig.app.json'],
      tsconfigRootDir: import.meta.dirname,
    },
  },
})
```

- Replace `tseslint.configs.recommended` to `tseslint.configs.recommendedTypeChecked` or `tseslint.configs.strictTypeChecked`
- Optionally add `...tseslint.configs.stylisticTypeChecked`
- Install [eslint-plugin-react](https://github.com/jsx-eslint/eslint-plugin-react) and update the config:

```js
// eslint.config.js
import react from 'eslint-plugin-react'

export default tseslint.config({
  // Set the react version
  settings: { react: { version: '18.3' } },
  plugins: {
    // Add the react plugin
    react,
  },
  rules: {
    // other rules...
    // Enable its recommended rules
    ...react.configs.recommended.rules,
    ...react.configs['jsx-runtime'].rules,
  },
})
```
