# Terminal Launch Commands

## Launch larql-server (backend)

```bash
# From larql-main root
cd /home/arty/Documents/projects/encoding-experiment/larql-main

# Build server
cargo build --release -p larql-server

# Launch server with vindex
./target/release/larql-server ${LARQL_VINDEX__PATH:-data/bitnet_b1_58-large/vindex} --port 8080 --cors
```

Server will listen on: http://0.0.0.0:8080

## Launch chuk-kv-anatomist UI (frontend)

```bash
# From chuk-kv-anatomist directory
cd apps/chuk-kv-anatomist

# Set environment variables for native backend
export VITE_SEARCH_BACKEND=native
export VITE_NATIVE_SEARCH_URL=http://127.0.0.1:8080

# Install dependencies (first time only)
npm install

# Launch dev server
npm run dev
```

UI will be available at: http://localhost:5173

## Alternative: MCP backend

```bash
# Set environment variables for MCP backend
export VITE_SEARCH_BACKEND=mcp
export VITE_MCP_URL=http://localhost:8765

# Launch dev server
npm run dev
```

## Test tools endpoint directly

```bash
# Test get_model_info
curl -X POST http://127.0.0.1:8080/tools/call \
  -H 'Content-Type: application/json' \
  -d '{"name":"get_model_info","arguments":{}}'

# Test context_map
curl -X POST http://127.0.0.1:8080/tools/call \
  -H 'Content-Type: application/json' \
  -d '{"name":"context_map","arguments":{"prompt":"hello","layer":1,"top_k":5}}'
```

## Quick one-liner

```bash
# Terminal 1: Server
cd /home/arty/Documents/projects/encoding-experiment/larql-main && \
./target/release/larql-server ${LARQL_VINDEX__PATH:-data/bitnet_b1_58-large/vindex} --port 8080 --cors

# Terminal 2: UI
cd /home/arty/Documents/projects/encoding-experiment/larql-main/apps/chuk-kv-anatomist && \
export VITE_SEARCH_BACKEND=native && \
export VITE_NATIVE_SEARCH_URL=http://127.0.0.1:8080 && \
npm run dev
```
