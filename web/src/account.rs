use sycamore::prelude::*;

use rollback::account::{Account, AccountId};
use rollback::total::Total;

#[component(CoAccounts<G>)]
pub fn co_accounts(accounts_vec: ReadSignal<Vec<(AccountId, Account)>>) -> View<G> {
    view! {
        div(class="section") {
            div(class="container") {
                Keyed(KeyedProps {
                    iterable: accounts_vec,
                    template: account_card,
                    key: |(id, _)| (id.clone()) ,
                })
            }
        }
    }
}

fn account_card<G>(account: (AccountId, Account)) -> View<G>
where
    G: sycamore::generic_node::GenericNode,
{
    let (id, account) = account;
    view! {
        div(class="card") {
            header(class="card-header") {
                p(class="card-header-title") {
                    (id)
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
    }
}
