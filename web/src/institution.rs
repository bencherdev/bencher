use sycamore::prelude::*;

use rollback::institution::Institution;

#[component(CoInstitutions<G>)]
pub fn institutions(institutions_vec: ReadSignal<Vec<Institution>>) -> View<G> {
    view! {
        div(class="section") {
            div(class="container") {
                Keyed(KeyedProps {
                    iterable: institutions_vec,
                    template: |i| view! {
                        div(class="card") {
                            (i)
                        }
                    },
                    key: |i| (i.clone()) ,
                })
            }
        }
    }
}
