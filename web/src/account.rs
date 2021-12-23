use sycamore::prelude::*;

use rollback::account::Account;
use rollback::total::Total;

#[component(CoAccounts<G>)]
pub fn co_accounts(accounts_vec: ReadSignal<Vec<Account>>) -> View<G> {
    view! {
        div(class="section") {
            div(class="container") {
                Keyed(KeyedProps {
                    iterable: accounts_vec,
                    template: account_card,
                    key: |acc| (acc.id()) ,
                })
            }
        }
    }
}

fn account_card<G>(account: Account) -> View<G>
where
    G: sycamore::generic_node::GenericNode,
{
    view! {
        div(class="card") {
            header(class="card-header") {
                p(class="card-header-title") {
                    (account.kind().clone())
                }

                p(class="card-header-icon") {
                    (account.total())
                }

                button(class="card-header-icon") {
                    span(class="icon") {
                        i(class="fas fa-angle-down")
                    }
                }
            }

            div(class="card-content") {
                div(class="content") {
                    "Funds"
                }
            }
        }

        br()
    }
}
