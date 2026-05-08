use larql_factboard::{project_factoid_board, sample_fact_records, FactoidBoardProjection};
use leptos::*;
use leptos_meta::*;
use leptos_router::*;

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Title text="LARQL Leptos - Batch DLA Scan"/>
        <Router>
            <main class="container">
                <Routes>
                    <Route path="/" view=FactoidBoard/>
                </Routes>
            </main>
        </Router>
    }
}

pub fn main() {
    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();
    leptos::mount_to_body(App);
}

#[component]
fn FactoidBoard() -> impl IntoView {
    let projection: FactoidBoardProjection = project_factoid_board(&sample_fact_records());

    view! {
        <section class="factboard-root">
            <header class="factboard-header">
                <h1>"Factoid Board"</h1>
                <p>"Rust-owned facts projected into UI (no runtime free prose authority)."</p>
                <button class="brand-button">"Refresh Projection"</button>
            </header>
            <div class="location-grid">
                {projection.location_groups.into_iter().map(|location| {
                    view! {
                        <article class="location-card">
                            <h2>{location.location}</h2>
                            {location.features.into_iter().map(|feature| {
                                view! {
                                    <section class="feature-block">
                                        <h3>{feature.feature}</h3>
                                        {feature.factoids.into_iter().map(|factoid| {
                                            view! {
                                                <div class="factoid-card">
                                                    <h4>{factoid.title}</h4>
                                                    <p class="factoid-summary">{factoid.summary}</p>
                                                    <ul class="rust-paths">
                                                        {factoid.rust_paths.into_iter().map(|path| {
                                                            view! {
                                                                <li>
                                                                    <strong>{path.path}</strong>
                                                                    <ul class="symbols">
                                                                        {path.symbols.into_iter().map(|symbol| {
                                                                            view! {
                                                                                <li>{format!("{:?}: {}", symbol.kind, symbol.name)}</li>
                                                                            }
                                                                        }).collect_view()}
                                                                    </ul>
                                                                </li>
                                                            }
                                                        }).collect_view()}
                                                    </ul>
                                                </div>
                                            }
                                        }).collect_view()}
                                    </section>
                                }
                            }).collect_view()}
                        </article>
                    }
                }).collect_view()}
            </div>
        </section>
    }
}
