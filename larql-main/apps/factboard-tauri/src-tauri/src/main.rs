use larql_factboard::{project_factoid_board, sample_fact_records, FactoidBoardProjection};

#[tauri::command]
fn factoid_board_projection() -> FactoidBoardProjection {
    project_factoid_board(&sample_fact_records())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![factoid_board_projection])
        .run(tauri::generate_context!())
        .expect("tauri app failed");
}
