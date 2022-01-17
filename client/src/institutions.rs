use url::Url;
use yew::{function_component, html, use_state, Callback, Html, Properties};

use rollback::account::{Account, AccountKind, Accounts};
use rollback::institution::{Institution, Institutions};
use rollback::ticker::TickerSymbols;

#[derive(Properties, PartialEq)]
pub struct InstitutionsListProps {
    pub institutions: Institutions,
}

#[function_component(InstitutionsList)]
pub fn institutions_list(props: &InstitutionsListProps) -> Html {
    html! {
        <div id="institutions">
            {
                props.institutions.iter().map(|(institution, _accounts)| {
                    html!{<div key={ institution.name() }>{ institution.name() }</div>}
                }).collect::<Html>()
            }
        </div>
    }
}
