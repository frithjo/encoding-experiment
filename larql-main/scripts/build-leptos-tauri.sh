#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root/crates/larql-leptos"

env -u NO_COLOR trunk build --public-url ./

html="$repo_root/crates/larql-leptos/dist/index.html"

perl -0pi -e 's/const wasm = await init\(\{ module_or_path: ([^}]+) \}\);/(async () => {\nconst wasm = await init({ module_or_path: $1 });/s; s/dispatchEvent\(new CustomEvent\("TrunkApplicationStarted", \{detail: \{wasm\}\}\)\);\n\n<\/script>/dispatchEvent(new CustomEvent("TrunkApplicationStarted", {detail: {wasm}}));\n})().catch((error) => { console.error("LARQL WASM bootstrap failed", error); const fallback = document.getElementById("wasm-fallback"); if (fallback) { const status = fallback.querySelector(".status p"); if (status) status.textContent = "Rust\/WASM frontend failed to load. See terminal\/webview logs."; } });\n\n<\/script>/s' "$html"

# Trunk injects a wasm preload hint that can trigger noisy "preloaded but not used"
# warnings in this runtime due request-mode mismatch; remove only that hint.
perl -0pi -e 's#<link rel="preload" href="\./larql-leptos-[^"]+_bg\.wasm"[^>]*>##g' "$html"
