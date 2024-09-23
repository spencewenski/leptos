use leptos::prelude::*;
mod api;
mod routes;
use leptos_meta::{provide_meta_context, Link, Meta, MetaTags, Stylesheet};
use leptos_router::{
    components::{FlatRoutes, Route, Router, RoutingProgress},
    ParamSegment, StaticSegment,
};
use routes::{nav::*, stories::*, story::*, users::*};
use std::time::Duration;
use leptos::server_fn::codec::GetUrl;

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[server(input = GetUrl)]
async fn foo() -> Result<String, ServerFnError> {
    let _options = expect_context::<LeptosOptions>();

    Ok("Foo".to_string())
}

#[derive(Clone)]
struct FooContext(Resource<Result<String, ServerFnError>>);

fn provide_foo_context() {
    let foo = Resource::new(
        move || (),
        move |_| async move {
            foo().await
        },
    );

    provide_context(FooContext(foo));
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    provide_foo_context();
    let (is_routing, set_is_routing) = signal(false);

    view! {
        <Stylesheet id="leptos" href="/pkg/hackernews_axum.css"/>
        <Link rel="shortcut icon" type_="image/ico" href="/favicon.ico"/>
        <Meta name="description" content="Leptos implementation of a HackerNews demo."/>
        <Router set_is_routing>
            // shows a progress bar while async data are loading
            <div class="routing-progress">
                <RoutingProgress is_routing max_time=Duration::from_millis(250)/>
            </div>
            <Nav />
            <main>
                <FlatRoutes fallback=|| "Not found.">
                    <Route path=(StaticSegment("users"), ParamSegment("id")) view=User/>
                    <Route path=(StaticSegment("stories"), ParamSegment("id")) view=Story/>
                    <Route path=ParamSegment("stories") view=Stories/>
                    // TODO allow optional params without duplication
                    <Route path=StaticSegment("") view=Stories/>
                </FlatRoutes>
            </main>
        </Router>
    }
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
