use sycamore::prelude::*;

use rollback::institution::Institution;

#[component(CoInstitutions<G>)]
pub fn institutions(institutions_vec: ReadSignal<Vec<Institution>>) -> View<G> {
    view! {
        ul {
            Keyed(KeyedProps {
                iterable: institutions_vec,
                template: |i| view! {
                    li { (i) }
                },
                key: |i| (i.clone()) ,
            })
        }
    }
}
