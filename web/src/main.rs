use sycamore::prelude::*;

fn main() {
    sycamore::render(|| view! {
        p { "Hello, World!" }
    });
}