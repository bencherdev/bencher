use sycamore::prelude::*;

use rollback::account::Account;
use rollback::investment::Investment;
use rollback::total::Total;

use crate::investment::CoInvestments;

#[component(CoAccounts<G>)]
pub fn co_accounts(accounts_vec: ReadSignal<Vec<Account>>) -> View<G> {
    view! {
        div(class="section") {
            div(class="container") {
                Keyed(KeyedProps {
                    iterable: accounts_vec,
                    template: account_card,
                    key: |acc| (acc.id().clone()) ,
                })
            }
        }
    }
}

fn account_card<G>(account: Account) -> View<G>
where
    G: sycamore::generic_node::GenericNode + sycamore::generic_node::Html,
{
    let investments_vec = create_memo(cloned!(account => move ||
        account.investments().values().cloned().collect::<Vec<Investment>>()));

    let account_kind = account.kind().clone();
    view! {
        div(class="card") {
            header(class="card-header") {
                p(class="card-header-title") {
                    (account_kind)
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
                    CoInvestments(investments_vec)
                }
            }
        }

        br()
    }
}
