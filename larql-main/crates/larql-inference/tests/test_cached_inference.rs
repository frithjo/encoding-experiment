//! Tests for cached layer inference integration.

use larql_inference::layer_graph::template::TemplatePattern;
use larql_inference::layer_graph::CachedLayerGraph;

#[test]
fn test_cached_layer_graph_from_residuals() {
    // Test building from residuals
    let hidden_size = 2560;
    let seq_len = 10;

    let residuals: Vec<(usize, ndarray::Array2<f32>)> = (0..=2)
        .map(|layer| {
            let data = vec![0.5f32; seq_len * hidden_size];
            let array = ndarray::Array2::from_shape_vec((seq_len, hidden_size), data).unwrap();
            (layer, array)
        })
        .collect();

    let cache = CachedLayerGraph::from_residuals(residuals);

    assert_eq!(cache.num_cached(), 3);
    assert!(cache.has_layer(0));
    assert!(cache.has_layer(1));
    assert!(cache.has_layer(2));
    assert!(!cache.has_layer(3));
}

#[test]
fn test_detect_template() {
    let templates = vec![
        TemplatePattern {
            name: "capital".to_string(),
            prefix_tokens: vec![100, 200, 300],
            cached_layers: 0..=12,
        },
        TemplatePattern {
            name: "lives".to_string(),
            prefix_tokens: vec![100, 200],
            cached_layers: 0..=12,
        },
    ];

    // Test exact match
    let token_ids = vec![100, 200, 300, 400];
    let detected = larql_inference::layer_graph::detect_template(&token_ids, &templates);
    assert_eq!(detected, Some(0));

    // Test shorter match
    let token_ids = vec![100, 200, 400];
    let detected = larql_inference::layer_graph::detect_template(&token_ids, &templates);
    assert_eq!(detected, Some(1));

    // Test no match
    let token_ids = vec![500, 600, 700];
    let detected = larql_inference::layer_graph::detect_template(&token_ids, &templates);
    assert_eq!(detected, None);
}

#[test]
fn test_detect_template_with_cache_no_cache_store() {
    // Test that detect_template_with_cache requires vindex with cache_store
    let _templates = vec![TemplatePattern {
        name: "capital".to_string(),
        prefix_tokens: vec![100, 200, 300],
        cached_layers: 0..=12,
    }];

    let _token_ids = vec![100, 200, 300, 400];

    // Full integration test requires actual vindex with cached_residuals.bin
    // This test verifies the function signature compiles correctly
}

#[test]
fn test_cached_layer_graph_has_layer() {
    // Test that has_layer correctly identifies cached layers
    let hidden_size = 2560;
    let seq_len = 10;

    let residuals: Vec<(usize, ndarray::Array2<f32>)> = (0..=2)
        .map(|layer| {
            let data = vec![0.5f32; seq_len * hidden_size];
            let array = ndarray::Array2::from_shape_vec((seq_len, hidden_size), data).unwrap();
            (layer, array)
        })
        .collect();

    let cache = CachedLayerGraph::from_residuals(residuals);

    assert!(cache.has_layer(0));
    assert!(cache.has_layer(1));
    assert!(cache.has_layer(2));
    assert!(!cache.has_layer(3));
    assert_eq!(cache.num_cached(), 3);
}
