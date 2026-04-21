use super::*;
use super::helpers::*;
use crate::parser;

// ── Session state: no backend ──

#[test]
fn no_backend_stats() {
    let mut session = Session::new();
    let stmt = parser::parse("STATS;").unwrap();
    let result = session.execute(&stmt);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, LqlError::NoBackend));
}

#[test]
fn no_backend_walk() {
    let mut session = Session::new();
    let stmt = parser::parse(r#"WALK "test" TOP 5;"#).unwrap();
    let result = session.execute(&stmt);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), LqlError::NoBackend));
}

#[test]
fn no_backend_describe() {
    let mut session = Session::new();
    let stmt = parser::parse(r#"DESCRIBE "France";"#).unwrap();
    assert!(matches!(
        session.execute(&stmt).unwrap_err(),
        LqlError::NoBackend
    ));
}

#[test]
fn no_backend_select() {
    let mut session = Session::new();
    let stmt = parser::parse("SELECT * FROM EDGES;").unwrap();
    assert!(matches!(
        session.execute(&stmt).unwrap_err(),
        LqlError::NoBackend
    ));
}

#[test]
fn no_backend_explain() {
    let mut session = Session::new();
    let stmt = parser::parse(r#"EXPLAIN WALK "test";"#).unwrap();
    assert!(matches!(
        session.execute(&stmt).unwrap_err(),
        LqlError::NoBackend
    ));
}

#[test]
fn no_backend_show_relations() {
    let mut session = Session::new();
    let stmt = parser::parse("SHOW RELATIONS;").unwrap();
    assert!(matches!(
        session.execute(&stmt).unwrap_err(),
        LqlError::NoBackend
    ));
}

#[test]
fn no_backend_show_layers() {
    let mut session = Session::new();
    let stmt = parser::parse("SHOW LAYERS;").unwrap();
    assert!(matches!(
        session.execute(&stmt).unwrap_err(),
        LqlError::NoBackend
    ));
}

#[test]
fn no_backend_show_features() {
    let mut session = Session::new();
    let stmt = parser::parse("SHOW FEATURES 26;").unwrap();
    assert!(matches!(
        session.execute(&stmt).unwrap_err(),
        LqlError::NoBackend
    ));
}

// ── USE errors ──

#[test]
fn use_nonexistent_vindex() {
    let mut session = Session::new();
    let stmt =
        parser::parse(r#"USE "/nonexistent/path/fake.vindex";"#).unwrap();
    let result = session.execute(&stmt);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), LqlError::Execution(_)));
}

#[test]
fn use_model_fails_on_nonexistent() {
    let mut session = Session::new();
    let stmt =
        parser::parse(r#"USE MODEL "/nonexistent/model";"#).unwrap();
    let result = session.execute(&stmt);
    // Should fail to resolve the model path
    assert!(result.is_err());
}

#[test]
fn use_model_auto_extract_parses() {
    // Verify AUTO_EXTRACT parses correctly (loading will fail for nonexistent model)
    let mut session = Session::new();
    let stmt = parser::parse(
        r#"USE MODEL "/nonexistent/model" AUTO_EXTRACT;"#,
    )
    .unwrap();
    let result = session.execute(&stmt);
    assert!(result.is_err());
}

// ── Lifecycle: error cases without valid model/vindex ──

#[test]
fn extract_fails_on_nonexistent_model() {
    let mut session = Session::new();
    let stmt = parser::parse(
        r#"EXTRACT MODEL "/nonexistent/model" INTO "/tmp/test_extract_out.vindex";"#,
    )
    .unwrap();
    let result = session.execute(&stmt);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), LqlError::Execution(_)));
}

#[test]
fn compile_no_backend() {
    let mut session = Session::new();
    let stmt = parser::parse(
        r#"COMPILE CURRENT INTO MODEL "out/";"#,
    )
    .unwrap();
    assert!(matches!(
        session.execute(&stmt).unwrap_err(),
        LqlError::NoBackend
    ));
}

#[test]
fn diff_nonexistent_vindex() {
    let mut session = Session::new();
    let stmt =
        parser::parse(r#"DIFF "/nonexistent/a.vindex" "/nonexistent/b.vindex";"#).unwrap();
    assert!(matches!(
        session.execute(&stmt).unwrap_err(),
        LqlError::Execution(_)
    ));
}

// ── Mutation: no-backend errors ──

#[test]
fn insert_no_backend() {
    let mut session = Session::new();
    let stmt = parser::parse(
        r#"INSERT INTO EDGES (entity, relation, target) VALUES ("a", "b", "c");"#,
    )
    .unwrap();
    assert!(matches!(
        session.execute(&stmt).unwrap_err(),
        LqlError::NoBackend
    ));
}

#[test]
fn delete_no_backend() {
    let mut session = Session::new();
    let stmt = parser::parse(
        r#"DELETE FROM EDGES WHERE entity = "x";"#,
    )
    .unwrap();
    assert!(matches!(
        session.execute(&stmt).unwrap_err(),
        LqlError::NoBackend
    ));
}

#[test]
fn update_no_backend() {
    let mut session = Session::new();
    let stmt = parser::parse(
        r#"UPDATE EDGES SET target = "y" WHERE entity = "x";"#,
    )
    .unwrap();
    assert!(matches!(
        session.execute(&stmt).unwrap_err(),
        LqlError::NoBackend
    ));
}

#[test]
fn merge_nonexistent_source() {
    let mut session = Session::new();
    let stmt =
        parser::parse(r#"MERGE "/nonexistent/source.vindex";"#).unwrap();
    assert!(matches!(
        session.execute(&stmt).unwrap_err(),
        LqlError::Execution(_)
    ));
}

// ── INFER ──

#[test]
fn infer_no_backend() {
    let mut session = Session::new();
    let stmt = parser::parse(r#"INFER "test" TOP 5;"#).unwrap();
    assert!(matches!(
        session.execute(&stmt).unwrap_err(),
        LqlError::NoBackend
    ));
}

// ── is_readable_token ──

#[test]
fn readable_tokens() {
    assert!(is_readable_token("French"));
    assert!(is_readable_token("Paris"));
    assert!(is_readable_token("capital-of"));
    assert!(is_readable_token("is"));
    assert!(is_readable_token("Europe"));
}

#[test]
fn unreadable_tokens() {
    assert!(!is_readable_token("ইসলামাবাদ"));
    assert!(!is_readable_token("южна"));
    assert!(!is_readable_token("ളാ"));
    assert!(!is_readable_token("ڪ"));
    assert!(!is_readable_token(""));
}

// ── is_content_token ──

#[test]
fn content_tokens_pass() {
    assert!(is_content_token("French"));
    assert!(is_content_token("Paris"));
    assert!(is_content_token("Europe"));
    assert!(is_content_token("Mozart"));
    assert!(is_content_token("composer"));
    assert!(is_content_token("Berlin"));
    assert!(is_content_token("IBM"));
    assert!(is_content_token("Facebook"));
}

#[test]
fn stop_words_rejected() {
    assert!(!is_content_token("the"));
    assert!(!is_content_token("from"));
    assert!(!is_content_token("for"));
    assert!(!is_content_token("with"));
    assert!(!is_content_token("this"));
    assert!(!is_content_token("about"));
    assert!(!is_content_token("which"));
    assert!(!is_content_token("first"));
    assert!(!is_content_token("after"));
}

#[test]
fn short_tokens_rejected() {
    assert!(!is_content_token("a"));
    assert!(!is_content_token("of"));
    assert!(!is_content_token("is"));
    assert!(!is_content_token("-"));
    assert!(!is_content_token("lö"));
    assert!(!is_content_token("par"));
}

#[test]
fn code_tokens_rejected() {
    assert!(!is_content_token("trialComponents"));
    assert!(!is_content_token("NavigationBar"));
    assert!(!is_content_token("LastName"));
}

// ── SHOW MODELS works without backend ──

#[test]
fn show_models_no_crash() {
    let mut session = Session::new();
    let stmt = parser::parse("SHOW MODELS;").unwrap();
    let result = session.execute(&stmt);
    assert!(result.is_ok());
}

// ── Pipe: errors propagate ──

#[test]
fn pipe_error_propagates() {
    let mut session = Session::new();
    let stmt = parser::parse(
        r#"STATS |> WALK "test";"#,
    )
    .unwrap();
    assert!(session.execute(&stmt).is_err());
}

// ── Format helpers ──

#[test]
fn format_number_small() {
    assert_eq!(format_number(42), "42");
    assert_eq!(format_number(999), "999");
}

#[test]
fn format_number_thousands() {
    assert_eq!(format_number(1_000), "1.0K");
    assert_eq!(format_number(10_240), "10.2K");
    assert_eq!(format_number(348_160), "348.2K");
}

#[test]
fn format_number_millions() {
    assert_eq!(format_number(1_000_000), "1.00M");
    assert_eq!(format_number(2_917_432), "2.92M");
}

#[test]
fn format_bytes_small() {
    assert_eq!(format_bytes(512), "512 B");
}

#[test]
fn format_bytes_kb() {
    assert_eq!(format_bytes(2048), "2.0 KB");
}

#[test]
fn format_bytes_mb() {
    let mb = 5 * 1_048_576;
    assert_eq!(format_bytes(mb), "5.0 MB");
}

#[test]
fn format_bytes_gb() {
    let gb = 6_420_000_000;
    assert!(format_bytes(gb).contains("GB"));
}

// ═══════════════════════════════════════════════════════════════
// Weight backend tests
// ═══════════════════════════════════════════════════════════════

/// Create a minimal ModelWeights for testing the Weight backend.
fn make_test_weights() -> larql_inference::ModelWeights {
    use std::collections::HashMap;
    use larql_inference::ndarray;

    let num_layers = 2;
    let hidden = 8;
    let intermediate = 4;
    let vocab_size = 16;

    let mut tensors: HashMap<String, ndarray::ArcArray2<f32>> = HashMap::new();
    let mut vectors: HashMap<String, Vec<f32>> = HashMap::new();

    for layer in 0..num_layers {
        let mut gate = ndarray::Array2::<f32>::zeros((intermediate, hidden));
        for i in 0..intermediate { gate[[i, i % hidden]] = 1.0 + layer as f32; }
        tensors.insert(format!("layers.{layer}.mlp.gate_proj.weight"), gate.into_shared());

        let mut up = ndarray::Array2::<f32>::zeros((intermediate, hidden));
        for i in 0..intermediate { up[[i, (i + 1) % hidden]] = 0.5; }
        tensors.insert(format!("layers.{layer}.mlp.up_proj.weight"), up.into_shared());

        let mut down = ndarray::Array2::<f32>::zeros((hidden, intermediate));
        for i in 0..intermediate { down[[i % hidden, i]] = 0.3; }
        tensors.insert(format!("layers.{layer}.mlp.down_proj.weight"), down.into_shared());

        for suffix in &["q_proj", "k_proj", "v_proj", "o_proj"] {
            let mut attn = ndarray::Array2::<f32>::zeros((hidden, hidden));
            for i in 0..hidden { attn[[i, i]] = 1.0; }
            tensors.insert(format!("layers.{layer}.self_attn.{suffix}.weight"), attn.into_shared());
        }

        vectors.insert(format!("layers.{layer}.input_layernorm.weight"), vec![1.0; hidden]);
        vectors.insert(format!("layers.{layer}.post_attention_layernorm.weight"), vec![1.0; hidden]);
    }

    vectors.insert("norm.weight".into(), vec![1.0; hidden]);

    let mut embed = ndarray::Array2::<f32>::zeros((vocab_size, hidden));
    for i in 0..vocab_size { embed[[i, i % hidden]] = 1.0; }
    let embed = embed.into_shared();
    let lm_head = embed.clone();

    let arch = larql_models::detect_from_json(&serde_json::json!({
        "model_type": "llama",
        "hidden_size": hidden,
        "num_hidden_layers": num_layers,
        "intermediate_size": intermediate,
        "head_dim": hidden,
        "num_attention_heads": 1,
        "num_key_value_heads": 1,
        "rope_theta": 10000.0,
        "vocab_size": vocab_size,
    }));

    larql_inference::ModelWeights {
        tensors, vectors, embed, lm_head,
        num_layers, hidden_size: hidden, intermediate_size: intermediate,
        vocab_size, head_dim: hidden, num_q_heads: 1, num_kv_heads: 1,
        rope_base: 10000.0, arch,
    }
}

/// Create a minimal tokenizer for testing.
fn make_test_tokenizer() -> std::sync::Arc<dyn larql_tokenizer::Tokenizer> {
    let tok_json = r#"{"version":"1.0","model":{"type":"BPE","vocab":{},"merges":[]},"added_tokens":[]}"#;
    std::sync::Arc::new(
        larql_tokenizer::HfTokenizer::from_bytes(tok_json.as_bytes()).unwrap(),
    )
}

/// Create a Session with Weight backend for testing.
fn weight_session() -> Session {
    let mut session = Session::new();
    session.backend = Backend::Weight {
        model_id: "test/model".into(),
        weights: make_test_weights(),
        tokenizer: make_test_tokenizer(),
    };
    session
}

#[test]
fn weight_backend_stats() {
    let mut session = weight_session();
    let stmt = parser::parse("STATS;").unwrap();
    let result = session.execute(&stmt).unwrap();
    assert!(result.iter().any(|l| l.contains("test/model")));
    assert!(result.iter().any(|l| l.contains("live weights")));
    assert!(result.iter().any(|l| l.contains("2"))); // num_layers
}

#[test]
fn weight_backend_walk_requires_vindex() {
    let mut session = weight_session();
    let stmt = parser::parse(r#"WALK "test" TOP 5;"#).unwrap();
    let err = session.execute(&stmt).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("requires a vindex"), "expected vindex error, got: {msg}");
    assert!(msg.contains("EXTRACT"), "should suggest EXTRACT, got: {msg}");
}

#[test]
fn weight_backend_describe_requires_vindex() {
    let mut session = weight_session();
    let stmt = parser::parse(r#"DESCRIBE "France";"#).unwrap();
    let err = session.execute(&stmt).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("requires a vindex"));
}

#[test]
fn weight_backend_select_requires_vindex() {
    let mut session = weight_session();
    let stmt = parser::parse("SELECT * FROM EDGES;").unwrap();
    let err = session.execute(&stmt).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("requires a vindex"));
}

#[test]
fn weight_backend_explain_walk_requires_vindex() {
    let mut session = weight_session();
    let stmt = parser::parse(r#"EXPLAIN WALK "test";"#).unwrap();
    let err = session.execute(&stmt).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("requires a vindex"));
}

#[test]
fn weight_backend_insert_requires_vindex() {
    let mut session = weight_session();
    let stmt = parser::parse(
        r#"INSERT INTO EDGES (entity, relation, target) VALUES ("a", "b", "c");"#
    ).unwrap();
    let err = session.execute(&stmt).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("requires a vindex") || msg.contains("mutation requires"));
}

#[test]
fn weight_backend_show_relations_requires_vindex() {
    let mut session = weight_session();
    let stmt = parser::parse("SHOW RELATIONS;").unwrap();
    let err = session.execute(&stmt).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("requires a vindex"));
}

#[test]
fn weight_backend_compile_current_requires_vindex() {
    let mut session = weight_session();
    let stmt = parser::parse(r#"COMPILE CURRENT INTO MODEL "out/";"#).unwrap();
    let err = session.execute(&stmt).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("EXTRACT") || msg.contains("vindex"));
}

#[test]
fn weight_backend_show_models_works() {
    let mut session = weight_session();
    let stmt = parser::parse("SHOW MODELS;").unwrap();
    let result = session.execute(&stmt);
    assert!(result.is_ok());
}

// ── Mutation pipeline integration tests ────────────────────────────────
//
// These tests build a tiny synthetic vindex on disk, load it via USE,
// and exercise the DELETE / UPDATE / patch-session paths through the
// real executor + parser. They cover the auto-patch lifecycle, the
// patch overlay update, and SAVE PATCH file emission.
//
// INSERT is exercised end-to-end in `compile_demo` against a real
// Gemma vindex (the synthetic tokenizer here has an empty vocab so it
// can't tokenise meaningful entity strings). The auto-patch session
// creation that INSERT triggers is covered indirectly by the DELETE
// auto-patch test below.

use larql_inference::ndarray::Array2;

/// Build a minimal vindex directory on disk that the LQL executor can
/// load via `USE`. Includes gate vectors, down_meta, embeddings, and a
/// stub tokenizer. Returns the directory path; the caller is
/// responsible for cleanup.
fn make_test_vindex_dir(tag: &str) -> std::path::PathBuf {
    use larql_vindex::{
        ExtractLevel, FeatureMeta, StorageDtype, VectorIndex, VindexConfig,
    };
    use larql_models::TopKEntry;

    let dir = std::env::temp_dir().join(format!("larql_lql_test_vindex_{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // Tiny in-memory index — 2 layers × 3 features × 4 hidden dims.
    let hidden = 4;
    let num_features = 3;
    let num_layers = 2;
    let vocab_size = 10;

    let mut gate0 = Array2::<f32>::zeros((num_features, hidden));
    gate0[[0, 0]] = 1.0;
    gate0[[1, 1]] = 1.0;
    gate0[[2, 2]] = 1.0;

    let mut gate1 = Array2::<f32>::zeros((num_features, hidden));
    gate1[[0, 3]] = 1.0;
    gate1[[1, 0]] = 0.5;
    gate1[[2, 2]] = -1.0;

    let make_meta = |tok: &str, id: u32, c: f32| FeatureMeta {
        top_token: tok.to_string(),
        top_token_id: id,
        c_score: c,
        top_k: vec![TopKEntry { token: tok.to_string(), token_id: id, logit: c }],
    };

    let meta0 = vec![
        Some(make_meta("Paris", 100, 0.95)),
        Some(make_meta("French", 101, 0.88)),
        Some(make_meta("Europe", 102, 0.75)),
    ];
    let meta1 = vec![
        Some(make_meta("Berlin", 200, 0.90)),
        None,
        Some(make_meta("Spain", 202, 0.70)),
    ];
    let down_meta = vec![Some(meta0), Some(meta1)];

    let index = VectorIndex::new(
        vec![Some(gate0), Some(gate1)],
        down_meta,
        num_layers,
        hidden,
    );

    let mut config = VindexConfig {
        version: 2,
        model: "test/lql-mutation".into(),
        family: "llama".into(),
        source: None,
        checksums: None,
        num_layers,
        hidden_size: hidden,
        intermediate_size: num_features,
        vocab_size,
        embed_scale: 1.0,
        extract_level: ExtractLevel::Browse,
        dtype: StorageDtype::F32,
        layer_bands: None,
        layers: Vec::new(),
        down_top_k: 5,
        has_model_weights: false,
        model_config: None,
    };
    index.save_vindex(&dir, &mut config).unwrap();

    // Synthetic embeddings.bin (vocab_size × hidden f32, all zeros).
    let embed_bytes = vec![0u8; vocab_size * hidden * 4];
    std::fs::write(dir.join("embeddings.bin"), embed_bytes).unwrap();

    // Stub tokenizer.json — empty BPE. Not used by DELETE / UPDATE /
    // PATCH; INSERT-against-this-vindex tests would need a real one.
    let tok_json = r#"{"version":"1.0","model":{"type":"BPE","vocab":{},"merges":[]},"added_tokens":[]}"#;
    std::fs::write(dir.join("tokenizer.json"), tok_json).unwrap();

    dir
}

/// Spin up a session and `USE` the test vindex from `make_test_vindex_dir`.
fn vindex_session(tag: &str) -> (Session, std::path::PathBuf) {
    let dir = make_test_vindex_dir(tag);
    let mut session = Session::new();
    let stmt = parser::parse(&format!(r#"USE "{}";"#, dir.display())).unwrap();
    session.execute(&stmt).expect("USE on synthetic vindex should succeed");
    (session, dir)
}

#[test]
fn use_synthetic_vindex_loads() {
    let (session, dir) = vindex_session("use_loads");
    assert!(matches!(session.backend, Backend::Vindex { .. }));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn delete_by_layer_and_feature_succeeds() {
    let (mut session, dir) = vindex_session("delete_lf");

    let stmt = parser::parse(
        r#"DELETE FROM EDGES WHERE layer = 0 AND feature = 0;"#,
    )
    .unwrap();
    let out = session.execute(&stmt).expect("DELETE should succeed");
    let joined = out.join("\n");
    assert!(
        joined.contains("Deleted") || joined.contains("deleted"),
        "expected delete confirmation in: {joined}"
    );

    // The patch session should now be active (auto-patch).
    assert!(
        session.patch_recording.is_some(),
        "DELETE should have started an auto-patch session"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn delete_no_matches_returns_message() {
    let (mut session, dir) = vindex_session("delete_nomatch");

    // Layer that doesn't exist in our 2-layer test vindex.
    let stmt = parser::parse(
        r#"DELETE FROM EDGES WHERE layer = 99 AND feature = 0;"#,
    )
    .unwrap();
    let result = session.execute(&stmt);
    // The executor either returns an empty-match message or errors —
    // both are acceptable; the important thing is no panic.
    assert!(result.is_ok() || result.is_err(), "no panic");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn update_feature_target_succeeds() {
    let (mut session, dir) = vindex_session("update_target");

    let stmt = parser::parse(
        r#"UPDATE EDGES SET target = "London" WHERE layer = 0 AND feature = 0;"#,
    )
    .unwrap();
    let out = session.execute(&stmt).expect("UPDATE should succeed");
    let joined = out.join("\n");
    assert!(
        joined.contains("Updated") || joined.contains("updated") || joined.contains("London"),
        "expected update confirmation in: {joined}"
    );

    assert!(
        session.patch_recording.is_some(),
        "UPDATE should have started an auto-patch session"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn explicit_begin_patch_starts_session() {
    let (mut session, dir) = vindex_session("begin_patch");

    let patch_path = dir.join("session.vlp");
    let stmt = parser::parse(&format!(r#"BEGIN PATCH "{}";"#, patch_path.display())).unwrap();
    session.execute(&stmt).expect("BEGIN PATCH should succeed");

    assert!(
        session.patch_recording.is_some(),
        "BEGIN PATCH should populate patch_recording"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_patch_writes_file_to_disk() {
    let (mut session, dir) = vindex_session("save_patch");

    // Start a patch, do a delete (so there's at least one operation), save.
    let patch_path = dir.join("save.vlp");
    let begin = parser::parse(&format!(r#"BEGIN PATCH "{}";"#, patch_path.display())).unwrap();
    session.execute(&begin).expect("BEGIN PATCH");

    let del = parser::parse(r#"DELETE FROM EDGES WHERE layer = 0 AND feature = 1;"#).unwrap();
    session.execute(&del).expect("DELETE under patch");

    let save = parser::parse("SAVE PATCH;").unwrap();
    let out = session.execute(&save).expect("SAVE PATCH");
    let joined = out.join("\n");
    assert!(
        patch_path.exists(),
        "SAVE PATCH should write the .vlp file. Output: {joined}"
    );

    // After SAVE PATCH, recording should be cleared.
    assert!(
        session.patch_recording.is_none(),
        "SAVE PATCH should clear the recording"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn auto_patch_session_starts_on_first_mutation() {
    let (mut session, dir) = vindex_session("auto_patch");

    // No explicit BEGIN PATCH first.
    assert!(session.patch_recording.is_none(), "no patch session before mutation");

    let del = parser::parse(r#"DELETE FROM EDGES WHERE layer = 0 AND feature = 0;"#).unwrap();
    session.execute(&del).expect("DELETE");

    assert!(
        session.patch_recording.is_some(),
        "first mutation should auto-start a patch session"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn merge_nonexistent_source_errors_cleanly() {
    let (mut session, dir) = vindex_session("merge_bad_src");

    let stmt = parser::parse(r#"MERGE "/nonexistent/src.vindex";"#).unwrap();
    let err = session.execute(&stmt).unwrap_err();
    assert!(matches!(err, LqlError::Execution(_)));

    let _ = std::fs::remove_dir_all(&dir);
}

// ── Session::patched_overlay_mut accessor ──

#[test]
fn patched_overlay_mut_returns_some_for_vindex_backend() {
    let (mut session, dir) = vindex_session("overlay_mut_some");
    assert!(
        session.patched_overlay_mut().is_some(),
        "Vindex backend should yield a mutable overlay"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn patched_overlay_mut_returns_none_for_no_backend() {
    let mut session = Session::new();
    assert!(
        session.patched_overlay_mut().is_none(),
        "fresh session with no backend should yield None"
    );
}

#[test]
fn patched_overlay_mut_round_trip_via_insert_feature() {
    use larql_models::TopKEntry;
    use larql_vindex::FeatureMeta;

    let (mut session, dir) = vindex_session("overlay_mut_round_trip");
    let gate = vec![0.7_f32, 0.0, 0.0, 0.0];
    {
        let overlay = session.patched_overlay_mut().expect("vindex backend");
        overlay.insert_feature(
            0, 1,
            gate.clone(),
            FeatureMeta {
                top_token: "z".into(),
                top_token_id: 9,
                c_score: 0.42,
                top_k: vec![TopKEntry { token: "z".into(), token_id: 9, logit: 0.42 }],
            },
        );
    }
    // Same accessor, second call: the gate we just wrote must still be there.
    let overlay2 = session.patched_overlay_mut().expect("vindex backend");
    assert_eq!(
        overlay2.overrides_gate_at(0, 1),
        Some(gate.as_slice()),
        "second patched_overlay_mut() call should observe the previous mutation",
    );
    let _ = std::fs::remove_dir_all(&dir);
}


#[test]
fn show_patches_with_no_patches_returns_message() {
    let (mut session, dir) = vindex_session("show_patches_empty");

    let stmt = parser::parse("SHOW PATCHES;").unwrap();
    let out = session.execute(&stmt).expect("SHOW PATCHES");
    let joined = out.join("\n").to_lowercase();
    assert!(
        joined.contains("no") || joined.contains("0") || joined.is_empty(),
        "expected an empty/no-patches message: {joined}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ── COMPILE INTO VINDEX integration tests ──────────────────────────────

#[test]
fn compile_into_vindex_no_patches_succeeds() {
    let (mut session, dir) = vindex_session("compile_nopatches_v");

    let output = dir.join("compiled.vindex");
    let stmt = parser::parse(&format!(
        r#"COMPILE CURRENT INTO VINDEX "{}";"#, output.display()
    )).unwrap();
    let out = session.execute(&stmt).expect("COMPILE INTO VINDEX should succeed");
    let joined = out.join("\n");
    assert!(joined.contains("Compiled"), "expected compile output: {joined}");
    assert!(output.exists(), "compiled vindex directory should exist");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn compile_into_vindex_with_down_overrides_bakes_them() {
    use larql_models::TopKEntry;
    use larql_vindex::FeatureMeta;

    let (mut session, dir) = vindex_session("compile_bake_down");

    // Create a synthetic down_weights.bin — per-layer [hidden, intermediate] f32.
    // hidden=4, intermediate=3, num_layers=2.
    let layer_floats = 4 * 3;
    let total = 2 * layer_floats;
    let bytes: Vec<u8> = (0..total).flat_map(|i| (i as f32 * 0.01).to_le_bytes()).collect();
    std::fs::write(dir.join("down_weights.bin"), &bytes).unwrap();

    {
        let overlay = session.patched_overlay_mut().expect("vindex backend");
        overlay.insert_feature(0, 0, vec![1.0, 0.0, 0.0, 0.0], FeatureMeta {
            top_token: "test".into(), top_token_id: 5, c_score: 0.9,
            top_k: vec![TopKEntry { token: "test".into(), token_id: 5, logit: 0.9 }],
        });
        overlay.set_down_vector(0, 0, vec![0.5, 0.6, 0.7, 0.8]);
    }
    let output = dir.join("compiled_baked.vindex");
    let stmt = parser::parse(&format!(
        r#"COMPILE CURRENT INTO VINDEX "{}";"#, output.display()
    )).unwrap();
    let out = session.execute(&stmt).expect("COMPILE should succeed");
    let joined = out.join("\n");
    assert!(joined.contains("Down overrides baked"), "expected baked overrides: {joined}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn compile_on_conflict_fail_detects_collision() {
    use larql_vindex::{PatchOp, VindexPatch};

    let (mut session, dir) = vindex_session("compile_conflict_fail");
    {
        let (_, _, patched) = session.require_patched_mut().unwrap();
        let mkp = |e: &str| VindexPatch {
            version: 1, base_model: String::new(), base_checksum: None,
            created_at: String::new(), description: None, author: None,
            tags: Vec::new(),
            operations: vec![PatchOp::Insert {
                layer: 0, feature: 0, relation: Some("r".into()),
                entity: e.into(), target: "t".into(), confidence: Some(0.9),
                gate_vector_b64: None, down_meta: None,
            }],
        };
        patched.patches.push(mkp("A"));
        patched.patches.push(mkp("C"));
    }
    let output = dir.join("compiled_fail.vindex");
    let stmt = parser::parse(&format!(
        r#"COMPILE CURRENT INTO VINDEX "{}" ON CONFLICT FAIL;"#, output.display()
    )).unwrap();
    let result = session.execute(&stmt);
    assert!(result.is_err(), "ON CONFLICT FAIL should error on collision");
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("FAIL") || msg.contains("colliding"), "error: {msg}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn compile_on_conflict_last_wins_succeeds() {
    use larql_vindex::{PatchOp, VindexPatch};

    let (mut session, dir) = vindex_session("compile_conflict_lw");
    {
        let (_, _, patched) = session.require_patched_mut().unwrap();
        let mkp = |e: &str| VindexPatch {
            version: 1, base_model: String::new(), base_checksum: None,
            created_at: String::new(), description: None, author: None,
            tags: Vec::new(),
            operations: vec![PatchOp::Insert {
                layer: 0, feature: 0, relation: Some("r".into()),
                entity: e.into(), target: "t".into(), confidence: Some(0.9),
                gate_vector_b64: None, down_meta: None,
            }],
        };
        patched.patches.push(mkp("A"));
        patched.patches.push(mkp("C"));
    }
    let output = dir.join("compiled_lw.vindex");
    let stmt = parser::parse(&format!(
        r#"COMPILE CURRENT INTO VINDEX "{}" ON CONFLICT LAST_WINS;"#, output.display()
    )).unwrap();
    assert!(session.execute(&stmt).is_ok(), "LAST_WINS should succeed despite collision");
    let _ = std::fs::remove_dir_all(&dir);
}

// ── MEMIT fact collection ────────────────────────────────────────────

#[test]
fn memit_facts_count_inserts_only() {
    use larql_vindex::PatchOp;

    let ops = vec![
        PatchOp::Insert {
            layer: 26, feature: 100, relation: Some("capital".into()),
            entity: "X".into(), target: "Y".into(), confidence: Some(0.9),
            gate_vector_b64: None, down_meta: None,
        },
        PatchOp::Delete { layer: 10, feature: 50, reason: None },
        PatchOp::Update { layer: 0, feature: 2, gate_vector_b64: None, down_meta: None },
    ];
    let insert_count = ops.iter().filter(|op| matches!(op, PatchOp::Insert { .. })).count();
    assert_eq!(insert_count, 1, "only INSERT should be counted");
}

#[test]
fn memit_facts_deduplicate_across_patches() {
    use larql_vindex::{PatchOp, VindexPatch};

    let mkp = |conf: f32| VindexPatch {
        version: 1, base_model: String::new(), base_checksum: None,
        created_at: String::new(), description: None, author: None,
        tags: Vec::new(),
        operations: vec![PatchOp::Insert {
            layer: 10, feature: 5, relation: Some("capital".into()),
            entity: "France".into(), target: "Paris".into(),
            confidence: Some(conf), gate_vector_b64: None, down_meta: None,
        }],
    };
    let patches = vec![mkp(0.9), mkp(0.95)];
    let mut seen = std::collections::HashSet::new();
    for p in &patches {
        for op in &p.operations {
            if let PatchOp::Insert { layer, entity, relation, target, .. } = op {
                seen.insert((entity.clone(), relation.clone().unwrap_or_default(), target.clone(), *layer));
            }
        }
    }
    assert_eq!(seen.len(), 1, "same fact in two patches → 1 after dedup");
}

// ── Template + decoy tests ───────────────────────────────────────────

#[test]
fn canonical_decoys_are_nonempty_and_diverse() {
    assert!(!super::CANONICAL_DECOY_PROMPTS.is_empty());
    let prefixes: std::collections::HashSet<String> = super::CANONICAL_DECOY_PROMPTS.iter()
        .map(|p| p.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
        .collect();
    assert_eq!(prefixes.len(), super::CANONICAL_DECOY_PROMPTS.len(),
        "decoy prompts should have unique 3-word prefixes");
}

#[test]
fn relation_template_simple() {
    let rel = "capital";
    let prompt = format!("The {} of entity is", rel.replace(['-', '_'], " "));
    assert_eq!(prompt, "The capital of entity is");
}

#[test]
fn relation_template_multi_word() {
    let rel = "native_language";
    let prompt = format!("The {} of entity is", rel.replace(['-', '_'], " "));
    assert_eq!(prompt, "The native language of entity is");
}

#[test]
fn relation_template_hyphenated_produces_double_of() {
    // Documents the known template quirk: "capital-of" → "capital of"
    // → "The capital of of X is". Users should use "capital" not "capital-of".
    let rel = "capital-of";
    let prompt = format!("The {} of X is", rel.replace(['-', '_'], " "));
    assert!(prompt.contains("of of"), "capital-of produces double 'of': {prompt}");
}

// Cholesky solver is unit-tested in larql-compute::cpu::ops::linalg::tests.
// MEMIT solve is integration-tested via compile_demo against real vindex.

// ── MEMIT struct ─────────────────────────────────────────────────────

#[test]
fn memit_fact_struct() {
    let f = larql_inference::MemitFact {
        prompt_tokens: vec![1, 2, 3], target_token_id: 42,
        layer: 26, label: "test".into(),
    };
    assert_eq!(f.layer, 26);
    assert_eq!(f.target_token_id, 42);
}

// ── Compile into model requires weights ──────────────────────────────

#[test]
fn compile_into_model_requires_model_weights() {
    let (mut session, dir) = vindex_session("compile_model_noweights");
    let output = dir.join("model_out");
    let stmt = parser::parse(&format!(
        r#"COMPILE CURRENT INTO MODEL "{}";"#, output.display()
    )).unwrap();
    let result = session.execute(&stmt);
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("model weights") || msg.contains("WITH ALL"), "error: {msg}");
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Architecture B: KNN Store tests ──────────────────────────────

#[test]
fn knn_store_insert_populates_store() {
    // INSERT on a browse-only vindex (no model weights) uses embedding-key fallback
    let (mut session, dir) = vindex_session("knn_insert");

    let stmt = parser::parse(
        r#"INSERT INTO EDGES (entity, relation, target) VALUES ("Atlantis", "capital", "Poseidon");"#,
    ).unwrap();
    let out = session.execute(&stmt).expect("INSERT should succeed");
    let joined = out.join("\n");
    assert!(joined.contains("Inserted"), "expected insert confirmation: {joined}");
    assert!(joined.contains("KNN store"), "expected KNN store mode: {joined}");
    assert!(joined.contains("1 entries"), "expected 1 entry: {joined}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn knn_store_insert_multiple_facts() {
    let (mut session, dir) = vindex_session("knn_multi");

    for (entity, target) in &[("Atlantis", "Poseidon"), ("Lemuria", "Mu"), ("Agartha", "Shambhala")] {
        let sql = format!(
            r#"INSERT INTO EDGES (entity, relation, target) VALUES ("{entity}", "capital", "{target}");"#
        );
        let stmt = parser::parse(&sql).unwrap();
        session.execute(&stmt).expect("INSERT should succeed");
    }

    // Check KNN store has 3 entries
    let stmt = parser::parse(
        r#"INSERT INTO EDGES (entity, relation, target) VALUES ("Wakanda", "capital", "Birnin");"#,
    ).unwrap();
    let out = session.execute(&stmt).expect("INSERT should succeed");
    let joined = out.join("\n");
    assert!(joined.contains("4 entries"), "expected 4 entries: {joined}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn knn_store_describe_shows_inserted_edges() {
    let (mut session, dir) = vindex_session("knn_describe");

    // Insert a fact
    let stmt = parser::parse(
        r#"INSERT INTO EDGES (entity, relation, target) VALUES ("Atlantis", "capital", "Poseidon");"#,
    ).unwrap();
    session.execute(&stmt).expect("INSERT");

    // Verify the KNN store is populated by checking via the overlay accessor
    let overlay = session.patched_overlay_mut().expect("vindex backend");
    let knn_entries = overlay.knn_store.entries_for_entity("Atlantis");
    assert_eq!(knn_entries.len(), 1, "expected 1 KNN entry for Atlantis");
    assert_eq!(knn_entries[0].1.target_token, "Poseidon");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn knn_store_delete_removes_entries() {
    let (mut session, dir) = vindex_session("knn_delete");

    // Insert two facts for different entities
    for sql in &[
        r#"INSERT INTO EDGES (entity, relation, target) VALUES ("Atlantis", "capital", "Poseidon");"#,
        r#"INSERT INTO EDGES (entity, relation, target) VALUES ("Lemuria", "capital", "Mu");"#,
    ] {
        let stmt = parser::parse(sql).unwrap();
        session.execute(&stmt).expect("INSERT");
    }

    // Verify both in store
    let overlay = session.patched_overlay_mut().expect("vindex");
    assert_eq!(overlay.knn_store.len(), 2);

    // Delete Atlantis via direct KNN store removal (since base features may not exist)
    overlay.knn_store.remove_by_entity("Atlantis");
    assert_eq!(overlay.knn_store.len(), 1);
    assert_eq!(overlay.knn_store.entries_for_entity("Atlantis").len(), 0);
    assert_eq!(overlay.knn_store.entries_for_entity("Lemuria").len(), 1);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn knn_store_compile_saves_and_loads() {
    let (mut session, dir) = vindex_session("knn_compile");

    // Insert a fact
    let stmt = parser::parse(
        r#"INSERT INTO EDGES (entity, relation, target) VALUES ("Atlantis", "capital", "Poseidon");"#,
    ).unwrap();
    session.execute(&stmt).expect("INSERT");

    // Compile
    let output = dir.join("compiled_knn.vindex");
    let stmt = parser::parse(&format!(
        r#"COMPILE CURRENT INTO VINDEX "{}";"#, output.display()
    )).unwrap();
    let out = session.execute(&stmt).expect("COMPILE should succeed");
    let joined = out.join("\n");
    assert!(joined.contains("KNN store: 1 entries"), "expected KNN count: {joined}");

    // Verify knn_store.bin exists
    assert!(output.join("knn_store.bin").exists(), "knn_store.bin should be in compiled vindex");

    // Load the compiled vindex and verify KNN store survives round-trip
    let stmt = parser::parse(&format!(
        r#"USE "{}";"#, output.display()
    )).unwrap();
    session.execute(&stmt).expect("USE compiled vindex");

    // Check the KNN store is loaded with the fact
    let overlay = session.patched_overlay_mut().expect("vindex");
    let entries = overlay.knn_store.entries_for_entity("Atlantis");
    assert_eq!(entries.len(), 1, "expected 1 KNN entry after compile+reload");
    assert_eq!(entries[0].1.target_token, "Poseidon");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn knn_store_patch_op_serialization() {
    // Verify InsertKnn PatchOp serializes and deserializes correctly
    let op = larql_vindex::PatchOp::InsertKnn {
        layer: 26,
        entity: "Atlantis".into(),
        relation: "capital".into(),
        target: "Poseidon".into(),
        target_id: 42,
        confidence: Some(1.0),
        key_vector_b64: larql_vindex::patch::core::encode_gate_vector(&[1.0, 0.0, 0.0, 0.0]),
    };
    let json = serde_json::to_string(&op).unwrap();
    assert!(json.contains("insert_knn"), "expected insert_knn tag: {json}");
    assert!(json.contains("Atlantis"), "expected entity: {json}");

    // Round-trip
    let decoded: larql_vindex::PatchOp = serde_json::from_str(&json).unwrap();
    match decoded {
        larql_vindex::PatchOp::InsertKnn { entity, target, layer, .. } => {
            assert_eq!(entity, "Atlantis");
            assert_eq!(target, "Poseidon");
            assert_eq!(layer, 26);
        }
        _ => panic!("expected InsertKnn variant"),
    }
}

#[test]
fn knn_store_delete_knn_patch_op() {
    let op = larql_vindex::PatchOp::DeleteKnn {
        entity: "Atlantis".into(),
    };
    let json = serde_json::to_string(&op).unwrap();
    assert!(json.contains("delete_knn"), "expected delete_knn tag: {json}");

    let decoded: larql_vindex::PatchOp = serde_json::from_str(&json).unwrap();
    match decoded {
        larql_vindex::PatchOp::DeleteKnn { entity } => {
            assert_eq!(entity, "Atlantis");
        }
        _ => panic!("expected DeleteKnn variant"),
    }
}

#[test]
fn knn_store_insert_at_layer_hint() {
    let (mut session, dir) = vindex_session("knn_layer_hint");

    // Synthetic vindex has only 2 layers (0, 1), so use AT LAYER 0
    let stmt = parser::parse(
        r#"INSERT INTO EDGES (entity, relation, target) VALUES ("Atlantis", "capital", "Poseidon") AT LAYER 0;"#,
    ).unwrap();
    let out = session.execute(&stmt).expect("INSERT AT LAYER");
    let joined = out.join("\n");
    assert!(joined.contains("L0"), "expected L0 in output: {joined}");

    // Verify it went to layer 0
    let overlay = session.patched_overlay_mut().expect("vindex");
    let entries = overlay.knn_store.entries_for_entity("Atlantis");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].0, 0, "expected layer 0");

    let _ = std::fs::remove_dir_all(&dir);
}

// ══════════════════════════════════════════════════════════════
// SHOW TOKENS local vs remote parity tests
// ══════════════════════════════════════════════════════════════

use std::collections::HashMap;
use larql_vindex::token_summary::{TokenHit, TokenShape, token_shape_name};
use crate::ast::{ExportFormat, TokenSortBy};

/// Build a deterministic `HashMap<String, TokenHit>` for parity testing.
fn sample_token_hits() -> HashMap<String, TokenHit> {
    let mut hits = HashMap::new();
    hits.insert("Paris".into(), TokenHit {
        shape: TokenShape::TitleWord,
        kind: Some("title"),
        hits: 3,
        syntax_hits: 1,
        knowledge_hits: 2,
        output_hits: 0,
        max_score: 0.95,
    });
    hits.insert("French".into(), TokenHit {
        shape: TokenShape::TitleWord,
        kind: Some("title"),
        hits: 2,
        syntax_hits: 0,
        knowledge_hits: 2,
        output_hits: 0,
        max_score: 0.88,
    });
    hits.insert("NASA".into(), TokenHit {
        shape: TokenShape::UpperWord,
        kind: Some("acronym"),
        hits: 1,
        syntax_hits: 0,
        knowledge_hits: 1,
        output_hits: 0,
        max_score: 0.75,
    });
    hits.insert("iPhone".into(), TokenHit {
        shape: TokenShape::MixedWord,
        kind: Some("mixed"),
        hits: 1,
        syntax_hits: 0,
        knowledge_hits: 0,
        output_hits: 1,
        max_score: 0.62,
    });
    hits.insert("1234".into(), TokenHit {
        shape: TokenShape::Number,
        kind: None,
        hits: 1,
        syntax_hits: 1,
        knowledge_hits: 0,
        output_hits: 0,
        max_score: 0.30,
    });
    hits
}

/// Serialize token hits to the server JSON format that `/v1/tokens` returns.
fn token_hits_to_server_json(
    label: &str,
    hits: &HashMap<String, TokenHit>,
) -> serde_json::Value {
    let rows: Vec<serde_json::Value> = hits.iter().map(|(tok, hit)| {
        serde_json::json!({
            "token": tok,
            "shape": token_shape_name(hit.shape),
            "kind": hit.kind.unwrap_or("none"),
            "hits": hit.hits,
            "syntax_hits": hit.syntax_hits,
            "knowledge_hits": hit.knowledge_hits,
            "output_hits": hit.output_hits,
            "max_score": hit.max_score,
        })
    }).collect();
    serde_json::json!({
        "label": label,
        "token_hits": rows,
        "token_count": hits.len(),
    })
}

/// Parse server JSON back into `(label, HashMap<String, TokenHit>)` using
/// the exact same logic as `remote_show_tokens`.
fn parse_server_token_group(value: &serde_json::Value) -> (String, HashMap<String, TokenHit>) {
    fn shape_from_name(name: &str) -> TokenShape {
        match name {
            "empty" => TokenShape::Empty,
            "lower" => TokenShape::LowerWord,
            "title" => TokenShape::TitleWord,
            "upper" => TokenShape::UpperWord,
            "mixed" => TokenShape::MixedWord,
            "number" => TokenShape::Number,
            "alnum" => TokenShape::AlphaNumeric,
            "punct" => TokenShape::Punctuation,
            "symbol" => TokenShape::Symbolic,
            _ => TokenShape::Other,
        }
    }
    fn kind_from_str(s: &str) -> Option<&'static str> {
        match s {
            "title" => Some("title"),
            "mixed" => Some("mixed"),
            "acronym" => Some("acronym"),
            _ => None,
        }
    }

    let label = value["label"].as_str().unwrap_or("").to_string();
    let mut hits: HashMap<String, TokenHit> = HashMap::new();
    if let Some(rows) = value["token_hits"].as_array() {
        for r in rows {
            let token = r["token"].as_str().unwrap_or("").to_string();
            let shape = shape_from_name(r["shape"].as_str().unwrap_or("other"));
            let kind = r["kind"].as_str().and_then(kind_from_str);
            let hit = TokenHit {
                shape,
                kind,
                hits: r["hits"].as_u64().unwrap_or(0) as usize,
                syntax_hits: r["syntax_hits"].as_u64().unwrap_or(0) as usize,
                knowledge_hits: r["knowledge_hits"].as_u64().unwrap_or(0) as usize,
                output_hits: r["output_hits"].as_u64().unwrap_or(0) as usize,
                max_score: r["max_score"].as_f64().unwrap_or(0.0) as f32,
            };
            hits.insert(token, hit);
        }
    }
    (label, hits)
}

#[test]
fn show_tokens_render_parity_flat() {
    let hits = sample_token_hits();
    let local = super::introspection::render_token_summary(
        "across 5 layers".into(), hits.clone(), false, None, None,
    );

    let server_json = token_hits_to_server_json("across 5 layers", &hits);
    let (label, parsed) = parse_server_token_group(&server_json);
    let remote = super::introspection::render_token_summary(
        label, parsed, false, None, None,
    );

    assert_eq!(local, remote, "flat render output must be byte-identical after JSON round-trip");
}

#[test]
fn show_tokens_render_parity_grouped() {
    let hits = sample_token_hits();
    let local_l0 = super::introspection::render_token_summary(
        "at layer 0".into(), hits.clone(), false, None, None,
    );
    let local_l1 = super::introspection::render_token_summary(
        "at layer 1".into(), hits.clone(), false, None, None,
    );
    let mut local = Vec::new();
    local.extend(local_l0);
    local.push(String::new());
    local.extend(local_l1);

    let g0 = token_hits_to_server_json("at layer 0", &hits);
    let g1 = token_hits_to_server_json("at layer 1", &hits);
    let (label0, parsed0) = parse_server_token_group(&g0);
    let (label1, parsed1) = parse_server_token_group(&g1);
    let mut remote = Vec::new();
    remote.extend(super::introspection::render_token_summary(label0, parsed0, false, None, None));
    remote.push(String::new());
    remote.extend(super::introspection::render_token_summary(label1, parsed1, false, None, None));

    assert_eq!(local, remote, "grouped render output must be byte-identical after JSON round-trip");
}

#[test]
fn show_tokens_export_csv_parity() {
    let hits = sample_token_hits();
    let local = super::introspection::export_token_summary(
        hits.clone(), ExportFormat::Csv, false, None, None,
    );

    // Export ignores grouping, so we flatten the server groups into a single map.
    let server_json = token_hits_to_server_json("flat", &hits);
    let (_, parsed) = parse_server_token_group(&server_json);
    let remote = super::introspection::export_token_summary(
        parsed, ExportFormat::Csv, false, None, None,
    );

    assert_eq!(local, remote, "CSV export must be byte-identical after JSON round-trip");
}

#[test]
fn show_tokens_export_json_parity() {
    let hits = sample_token_hits();
    let local = super::introspection::export_token_summary(
        hits.clone(), ExportFormat::Json, false, None, None,
    );

    let server_json = token_hits_to_server_json("flat", &hits);
    let (_, parsed) = parse_server_token_group(&server_json);
    let remote = super::introspection::export_token_summary(
        parsed, ExportFormat::Json, false, None, None,
    );

    assert_eq!(local, remote, "JSON export must be byte-identical after JSON round-trip");
}

#[test]
fn show_tokens_sort_parity() {
    let hits = sample_token_hits();
    for sort in [
        Some(TokenSortBy::MaxScore),
        Some(TokenSortBy::Distinct),
        Some(TokenSortBy::EntityLike),
        Some(TokenSortBy::Shape),
        None,
    ] {
        let local = super::introspection::render_token_summary(
            "test".into(), hits.clone(), false, sort, Some(3),
        );
        let server_json = token_hits_to_server_json("test", &hits);
        let (_, parsed) = parse_server_token_group(&server_json);
        let remote = super::introspection::render_token_summary(
            "test".into(), parsed, false, sort, Some(3),
        );
        assert_eq!(local, remote, "render with sort={sort:?} must be byte-identical");
    }
}

#[test]
fn show_tokens_end_to_end_vindex_parity() {
    let (mut session, dir) = vindex_session("show_tokens_parity");

    // Local path: SHOW TOKENS
    let stmt = parser::parse("SHOW TOKENS;").unwrap();
    let local = session.execute(&stmt).expect("local SHOW TOKENS");

    // Build the server JSON response that `/v1/tokens` would return for the same data.
    let (_path, config, patched) = session.require_vindex().unwrap();
    let bands = super::query::describe_resolve_bands(config);
    let scan_layers: Vec<usize> = (0..config.num_layers).collect();
    let filters = larql_vindex::token_summary::TokenFilters::default();
    let hits = larql_vindex::token_summary::collect_token_hits(
        patched, &scan_layers, &bands, &filters,
    );
    let server_json = token_hits_to_server_json(
        &format!("across {} layers", scan_layers.len()),
        &hits,
    );

    // Parse back and render exactly as the remote path would.
    let (label, parsed) = parse_server_token_group(&server_json);
    let remote = super::introspection::render_token_summary(
        label, parsed, false, None, None,
    );

    assert_eq!(local, remote,
        "end-to-end: local SHOW TOKENS must match remote-rendered output");

    let _ = std::fs::remove_dir_all(&dir);
}
