use leptos::prelude::*;
mod api;
mod routes;
use leptos_meta::{provide_meta_context, Link, Meta, MetaTags, Stylesheet};
use leptos_router::{
    components::{FlatRoutes, Route, Router},
    OptionalParamSegment, ParamSegment, StaticSegment,
};
use routes::{nav::*, stories::*, story::*, users::*};
#[cfg(feature = "ssr")]
pub mod fallback;

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options islands=true/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

/// Minimal reproduction of a hydration error.
///
/// Error:
///
/// ```plaintext
/// A hydration error occurred while trying to hydrate an element defined at src/lib.rs:79:10.
///
/// The framework expected an HTML <div> element, but found this instead:  
/// #text "Bar"
///     ...
///     parentElement: <div>
///     ...
///
/// The hydration mismatch may have occurred slightly earlier, but this is the first time the framework found a node of an unexpected type.
/// ```
///
/// Note the following:
/// - Parent (`Foo`) is an island
///     - Issue does not repro if `Foo` is a component
/// - Parent contains a `Transition` with a view rendered in a `Suspend`
///     - Issue does not repro if `Bar` and `Baz` are not in a `Transition` + `Suspend`
/// - `Suspend`'s view contains two islands
///     - Issue does not repro if only `Bar` is present
///     - Issue does not repro if either or both of `Bar`/`Baz` are components
///
/// Interestingly, and possibly related, the error is different if `Bar`/`Baz` are replaced with `Bar2`/`Baz2`:
///
/// ```plaintext
/// panicked at /home/spencer/code/leptos/tachys/src/html/mod.rs:188:14:
/// called `Option::unwrap()` on a `None` value
/// ```
#[island]
fn Foo() -> impl IntoView {
    view! {
        <Transition>
            {move || Suspend::new(async move {
                    view! {
                        <Bar/>
                        <Baz/>
                        // <Bar2/>
                        // <Baz2/>
                    }
            })}
        </Transition>
    }
}

#[island]
fn Bar() -> impl IntoView {
    view! {
        <div>"Bar"</div>
    }
}

#[island]
fn Baz() -> impl IntoView {
    view! {
        <div>"Baz"</div>
    }
}

#[island]
fn Bar2() -> impl IntoView {
    view! {
        <div><p>"Bar2"</p></div>
    }
}

#[island]
fn Baz2() -> impl IntoView {
    view! {
        <div><p>"Baz2"</p></div>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/hackernews.css"/>
        <Link rel="shortcut icon" type_="image/ico" href="/favicon.ico"/>
        <Meta name="description" content="Leptos implementation of a HackerNews demo."/>
        <Foo/>
    }
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_islands();
}
