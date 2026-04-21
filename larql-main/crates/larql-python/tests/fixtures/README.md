# UI test fixtures

## `ui_walk_trace_vindex/`

Minimal `.vindex` with **full model weights** (mmap attention/FFN/norms/lm_head, gate vectors, `down_meta.bin`) so `larql.WalkModel.trace` and the workbench **`/trace`** route can run end-to-end in CI.

Regenerate:

```bash
cd larql-main
rm -rf /tmp/ui_walk_trace_vindex
cargo run -p larql-vindex --bin ui_test_vindex -- /tmp/ui_walk_trace_vindex
rm -rf crates/larql-python/tests/fixtures/ui_walk_trace_vindex
cp -a /tmp/ui_walk_trace_vindex crates/larql-python/tests/fixtures/
```

Tokenizer and tensor layout match `crates/larql-vindex/src/bin/ui_test_vindex.rs` and `crates/larql-vindex/assets/ui_trace_tokenizer.json`.
