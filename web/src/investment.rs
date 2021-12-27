use sycamore::prelude::*;

use rollback::investment::Investment;
use rollback::total::Total;

#[component(CoInvestments<G>)]
pub fn co_investments(investments_vec: ReadSignal<Vec<Investment>>) -> View<G> {
    view! {
        div(class="section") {
            div(class="container") {
                Keyed(KeyedProps {
                    iterable: investments_vec,
                    template: investment_card,
                    key: |inv| (inv.fund().ticker_symbol().clone()) ,
                })
            }
        }
    }
}

fn investment_card<G>(investment: Investment) -> View<G>
where
    G: sycamore::generic_node::GenericNode,
{
    let fund = investment.fund().clone();
    view! {
        div(class="card") {
            header(class="card-header") {
                p(class="card-header-title") {
                    (fund)
                }

                p(class="card-header-icon") {
                    (investment.total())
                }

                button(class="card-header-icon") {
                    span(class="icon") {
                        i(class="fas fa-angle-down")
                    }
                }
            }

            // TODO add trades
            // div(class="card-content") {
            //     div(class="content") {
            //         "Trades"
            //     }
            // }
        }

        br()
    }
}
