use larql_workbench_core::{
    run_analyze_infer, run_bit_perfect_eraser, run_describe, run_lql_query, AnalyzeInferRequest,
    AnalyzeInferResponse, BitPerfectEraserRunRequest, BitPerfectEraserRunResponse,
    DescribeRunRequest, DescribeRunResponse, LqlRunRequest, LqlRunResponse,
};

#[tauri::command]
fn run_lql_query_command(request: LqlRunRequest) -> Result<LqlRunResponse, String> {
    run_lql_query(&request).map_err(|e| e.to_string())
}

#[tauri::command]
fn run_describe_command(request: DescribeRunRequest) -> Result<DescribeRunResponse, String> {
    run_describe(&request).map_err(|e| e.to_string())
}

#[tauri::command]
fn run_analyze_infer_command(request: AnalyzeInferRequest) -> Result<AnalyzeInferResponse, String> {
    run_analyze_infer(&request).map_err(|e| e.to_string())
}

#[tauri::command]
fn run_bit_perfect_eraser_command(
    request: BitPerfectEraserRunRequest,
) -> Result<BitPerfectEraserRunResponse, String> {
    run_bit_perfect_eraser(&request).map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            run_lql_query_command,
            run_describe_command,
            run_analyze_infer_command,
            run_bit_perfect_eraser_command
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Tauri shell");
}
