#[cfg(feature = "circuit-detect")]
mod tests {
    use larql_terminal_batch_dla::{BatchDlaResult, AttentionData, CircuitHighlight};

    #[test]
    fn test_circuit_highlight_mapping() {
        let mock_result = BatchDlaResult {
            attention: vec![
                AttentionData { layer: 0, heads: vec![vec![0.1; 5]] },
            ],
            logit_lens: None,
            head_dla: vec![],
            num_layers: 1,
            predictions: vec![],
            seq_len: 5,
            tokens: vec![1, 2, 3, 4, 5],
            strings: None,
            circuits: Some(vec![
                CircuitHighlight {
                    layer: 0,
                    head: 0,
                    from_token: 1,
                    to_token: 3,
                    color_rgb: (255, 0, 0),
                }
            ]),
            generation_trace: vec![],
            token_analysis: vec![],
            analysis_summary: None,
            ridge_by_layer: vec![],
        };
        
        let circuits = mock_result.circuits.as_ref().unwrap();
        assert_eq!(circuits.len(), 1);
        let h = &circuits[0];
        assert_eq!(h.layer, 0);
        assert_eq!(h.from_token, 1);
        assert_eq!(h.to_token, 3);
    }
}
