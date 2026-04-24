#[cfg(feature = "token-decode")]
mod tests {
    use larql_terminal_batch_dla::{App, BatchDlaResult, AttentionData};

    #[test]
    fn test_token_string_association() {
        let mut app = App::new();
        
        let mock_result = BatchDlaResult {
            attention: vec![
                AttentionData { layer: 0, heads: vec![vec![0.1; 2]] },
            ],
            logit_lens: None,
            head_dla: vec![],
            num_layers: 1,
            predictions: vec![],
            seq_len: 2,
            tokens: vec![101, 102],
            strings: Some(vec!["hello".to_string(), "world".to_string()]),
            circuits: None,
            generation_trace: vec![],
            token_analysis: vec![],
            analysis_summary: None,
            ridge_by_layer: vec![],
        };
        
        app.result = Some(mock_result);
        app.cursor_x = 0;
        app.cursor_y = 1;
        
        // This test mostly verifies that the data structure supports the strings
        // and we can access them based on cursor position.
        let res = app.result.as_ref().unwrap();
        let strs = res.strings.as_ref().unwrap();
        
        assert_eq!(strs[app.cursor_x], "hello");
        assert_eq!(strs[app.cursor_y], "world");
    }
}
