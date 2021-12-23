use sycamore::prelude::*;

use rollback::institution::{Institution, InstitutionAccounts};
use rollback::total::Total;

#[component(CoInstitutions<G>)]
pub fn co_institutions(
    institutions_vec: ReadSignal<Vec<(Institution, InstitutionAccounts)>>,
) -> View<G> {
    view! {
        div(class="section") {
            div(class="container") {
                Keyed(KeyedProps {
                    iterable: institutions_vec,
                    template: institution_card,
                    key: |i| (i.clone()) ,
                })
            }
        }
    }
}

pub fn institution_card<G>(institution: (Institution, InstitutionAccounts)) -> View<G>
where
    G: sycamore::generic_node::GenericNode,
{
    let (institution, accounts) = institution;
    view! {
        div(class="card") {
            header(class="card-header") {
                p(class="card-header-title") {
                    (institution)
                }

                p(class="card-header-icon") {
                    (accounts.total())
                }

                button(class="card-header-icon") {
                    span(class="icon") {
                        i(class="fas fa-angle-down")
                    }
                }
            }

            div(class="card-content") {
                div(class="content") {
                    "Accounts"
                }
            }
        }
    }
}
