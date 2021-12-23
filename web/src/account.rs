use sycamore::prelude::*;

use rollback::account::{Account, AccountId, Accounts};
use rollback::total::Total;

pub fn accounts<G>(accounts: Accounts) -> View<G>
where
    G: sycamore::generic_node::GenericNode + sycamore::generic_node::Html,
{
    let accounts_vec = create_memo(cloned!(accounts => move ||
        accounts.iter().map(|(id, acc)| (id.clone(), acc.clone())).collect::<Vec<(AccountId, Account)>>()));

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
