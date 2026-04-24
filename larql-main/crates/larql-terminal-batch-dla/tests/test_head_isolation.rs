#[cfg(feature = "head-isolation")]
mod tests {
    use larql_terminal_batch_dla::{App, Focus, BatchDlaResult, AttentionData};
    use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
    use tokio::runtime::Runtime;

    #[test]
    fn test_head_isolation_cycling() {
        let rt = Runtime::new().unwrap();
        let mut app = App::new();
        
        // Mock result with 2 layers, each with 2 heads
        let mock_result = BatchDlaResult {
            attention: vec![
                AttentionData { layer: 0, heads: vec![vec![0.1; 4], vec![0.2; 4]] },
                AttentionData { layer: 1, heads: vec![vec![0.3; 4], vec![0.4; 4]] },
            ],
            logit_lens: None,
            head_dla: vec![],
            num_layers: 2,
            predictions: vec![],
            seq_len: 4,
            tokens: vec![1, 2, 3, 4],
            strings: None,
            circuits: None,
            generation_trace: vec![],
            token_analysis: vec![],
            analysis_summary: None,
            ridge_by_layer: vec![],
        };
        
        app.result = Some(mock_result);
        app.focus = Focus::Image;
        app.selected_head = None; // Start with Avg
        
        let key_h = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
        
        // 1st press: Head 0
        app.handle_key(key_h, &rt, "");
        assert_eq!(app.selected_head, Some(0));
        
        // 2nd press: Head 1
        app.handle_key(key_h, &rt, "");
        assert_eq!(app.selected_head, Some(1));
        
        // 3rd press: Back to Avg (None)
        app.handle_key(key_h, &rt, "");
        assert_eq!(app.selected_head, None);
    }
}
