use sycamore::prelude::*;

use rollback::account::{Account, AccountId, Accounts};
use rollback::institution::Institution;
use rollback::total::Total;

use crate::account::CoAccounts;

#[component(CoInstitutions<G>)]
pub fn co_institutions(institutions_vec: ReadSignal<Vec<(Institution, Accounts)>>) -> View<G> {
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

fn institution_card<G>(item: (Institution, Accounts)) -> View<G>
where
    G: sycamore::generic_node::GenericNode + sycamore::generic_node::Html,
{
    let (institution, accounts) = item;

    let accounts_vec = create_memo(cloned!(accounts => move ||
        accounts.values().cloned().collect::<Vec<Account>>()));

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
                    CoAccounts(accounts_vec)
                }
            }
        }

        br()
    }
}
