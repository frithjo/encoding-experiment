#[cfg(feature = "logit-lens")]
mod tests {
    use larql_terminal_batch_dla::{App, BatchDlaResult, AttentionData};

    #[test]
    fn test_logit_lens_data() {
        let mut app = App::new();
        
        let mock_result = BatchDlaResult {
            attention: vec![
                AttentionData { layer: 0, heads: vec![vec![0.1; 1]] },
            ],
            logit_lens: None,
            head_dla: vec![],
            num_layers: 1,
            predictions: vec![("The".to_string(), 0.9), ("A".to_string(), 0.05)],
            seq_len: 1,
            tokens: vec![1],
            strings: None,
            circuits: None,
            generation_trace: vec![],
            token_analysis: vec![],
            analysis_summary: None,
            ridge_by_layer: vec![],
        };
        
        app.result = Some(mock_result);
        
        let res = app.result.as_ref().unwrap();
        assert_eq!(res.predictions.len(), 2);
        assert_eq!(res.predictions[0].0, "The");
        assert!((res.predictions[0].1 - 0.9).abs() < 1e-6);
    }
}
