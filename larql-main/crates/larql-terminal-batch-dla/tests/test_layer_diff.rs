#[cfg(feature = "layer-diff")]
mod tests {
    use larql_terminal_batch_dla::{App, Focus};
    use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
    use tokio::runtime::Runtime;

    #[test]
    fn test_layer_diff_toggle() {
        let rt = Runtime::new().unwrap();
        let mut app = App::new("http://localhost:8080".to_string());

        app.focus = Focus::Image;
        app.diff_mode = false;

        let key_f = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE);

        // 1st press: Diff On
        app.handle_key(key_f, &rt);
        assert!(app.diff_mode);

        // 2nd press: Diff Off
        app.handle_key(key_f, &rt);
        assert!(!app.diff_mode);
    }
}
