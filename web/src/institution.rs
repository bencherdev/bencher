use sycamore::prelude::*;

use rollback::account::{Account, Accounts};
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

                // TODO make these "Add" cards reactive.
                // The UI for this should be that the "Add" button becomes the form entry area
                // This form entry will have a "Cancel" and "Save" option
                // The "Save" option will only be clickable once all validation is complete
                // A valid save or "Cancel" will return to displaying a button.
                div(class="card") {
                    div(class="card-content") {
                        div(class="content") {
                            button {
                                ("Add Institution")
                            }
                        }
                    }
                }

                br()
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

            footer(class="card-footer") {
                button(class="card-footer-item") {
                    "Edit"
                }
            }
        }

        br()
    }
}
