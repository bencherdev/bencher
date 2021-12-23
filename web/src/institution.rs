use sycamore::prelude::*;

use rollback::institution::Institution;

#[component(CoInstitutions<G>)]
pub fn institutions(institutions_vec: ReadSignal<Vec<Institution>>) -> View<G> {
    view! {
        div(class="section") {
            div(class="container") {
                Keyed(KeyedProps {
                    iterable: institutions_vec,
                    template: institution,
                    key: |i| (i.clone()) ,
                })
            }
        }
    }
}

pub fn institution<G>(institution: Institution) -> View<G>
where
    G: sycamore::generic_node::GenericNode,
{
    view! {
        div(class="card") {
            (institution)
        }
    }
}
