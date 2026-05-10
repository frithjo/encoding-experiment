use leptos::*;

use super::*;

#[component]
pub(crate) fn LqlConsole() -> impl IntoView {
    let (workspace_path, set_workspace_path) = create_signal(String::new());
    let (query, set_query) = create_signal("STATS".to_string());
    let (loading, set_loading) = create_signal(false);
    let (error, set_error) = create_signal(None::<String>);
    let (lines, set_lines) = create_signal(Vec::<String>::new());
    let (history, set_history) = create_signal(Vec::<String>::new());
    let (snippets, set_snippets) = create_signal(vec![
        "STATS".to_string(),
        "DESCRIBE \"France\" KNOWLEDGE".to_string(),
        "DESCRIBE \"Atlantis\" ALL LAYERS VERBOSE".to_string(),
    ]);

    let run = move |_| {
        let workspace = workspace_path.get().trim().to_string();
        let statement = query.get().trim().to_string();

        set_loading.set(true);
        set_error.set(None);
        set_lines.set(Vec::new());

        wasm_bindgen_futures::spawn_local(async move {
            let req = LqlRunRequest {
                workspace_path: workspace,
                query: statement.clone(),
            };
            match invoke_tauri_lql(req).await {
                Ok(resp) => {
                    if !statement.is_empty() {
                        set_history.update(|items| {
                            items.retain(|item| item != &statement);
                            items.insert(0, statement.clone());
                            items.truncate(12);
                        });
                    }
                    set_lines.set(resp.lines);
                    set_loading.set(false);
                }
                Err(e) => {
                    set_error.set(Some(e));
                    set_loading.set(false);
                }
            }
        });
    };
    let save_snippet = move |_| {
        let statement = query.get().trim().to_string();
        if statement.is_empty() {
            return;
        }
        set_snippets.update(|items| {
            if !items.iter().any(|item| item == &statement) {
                items.insert(0, statement);
            }
            items.truncate(16);
        });
    };
    let clear = move |_| {
        set_lines.set(Vec::new());
        set_error.set(None);
    };

    view! {
        <>
            <WorkbenchChrome active_task="LQL Console"/>
            <section class="workspace">
                <div class="lql-grid">
                    <div class="panel">
                        <div class="eyebrow">"Projected query surface"</div>
                        <h1>"LQL Console"</h1>
                        <div class="field-stack">
                            <label>
                                "Workspace / vindex path"
                                <input
                                    type="text"
                                    prop:value=workspace_path
                                    on:input=move |e| set_workspace_path.set(event_target_value(&e))
                                    placeholder="/path/to/model.vindex"
                                />
                            </label>

                            <label>
                                "Query"
                                <textarea
                                    prop:value=query
                                    on:input=move |e| set_query.set(event_target_value(&e))
                                    rows="14"
                                />
                            </label>
                        </div>

                        <div class="toolbar">
                            <button class="primary-button" on:click=run disabled=loading>
                                {move || if loading.get() { "Running" } else { "Run" }}
                            </button>
                            <button class="ghost-button" on:click=save_snippet>"Save snippet"</button>
                            <button class="ghost-button" on:click=clear>"Clear result"</button>
                        </div>

                        {move || {
                            if let Some(err) = error.get() {
                                view! { <div class="error">{err}</div> }.into_view()
                            } else {
                                view! {}.into_view()
                            }
                        }}
                    </div>

                    <div class="result-panel">
                        <h2>"Projected result"</h2>
                        <div class="status-strip">
                            <div class="status-card">
                                <strong>"Runtime"</strong>
                                <span>"Tauri -> Rust LQL session"</span>
                            </div>
                            <div class="status-card">
                                <strong>"Lines"</strong>
                                <span>{move || lines.with(|items| items.len())}</span>
                            </div>
                        </div>
                        {move || {
                            let current = lines.get();
                            if current.is_empty() {
                                view! {
                                    <p class="muted">"Run a query to project output here."</p>
                                }
                                .into_view()
                            } else {
                                view! {
                                    <pre>{current.join("\n")}</pre>
                                }
                                .into_view()
                            }
                        }}
                    </div>
                </div>

                <div class="status-strip">
                    <details class="drawer-panel">
                        <summary>"Saved snippets"</summary>
                        <div class="snippet-list">
                            {move || snippets.get().into_iter().map(|snippet| {
                                let snippet_for_click = snippet.clone();
                                view! {
                                    <button class="ghost-button" on:click=move |_| set_query.set(snippet_for_click.clone())>
                                        {snippet}
                                    </button>
                                }
                            }).collect_view()}
                        </div>
                    </details>

                    <details class="drawer-panel">
                        <summary>"Query history"</summary>
                        <div class="snippet-list">
                            {move || {
                                let current = history.get();
                                if current.is_empty() {
                                    view! { <p class="muted">"No queries run in this session."</p> }.into_view()
                                } else {
                                    view! {
                                        <>
                                            {current.into_iter().map(|item| {
                                                let item_for_click = item.clone();
                                                view! {
                                                    <button class="ghost-button" on:click=move |_| set_query.set(item_for_click.clone())>
                                                        {item}
                                                    </button>
                                                }
                                            }).collect_view()}
                                        </>
                                    }.into_view()
                                }
                            }}
                        </div>
                    </details>

                    <details class="drawer-panel">
                        <summary>"Workspace browser"</summary>
                        <p class="muted">
                            "Schema and relation browsing is projected through LQL/Explorer commands. Use Tools -> Explorer Describe for focused entity inspection."
                        </p>
                    </details>
                </div>
            </section>
            <ArtifactBar status="projection: LQL query facade over Rust executor".to_string()/>
        </>
    }
}
