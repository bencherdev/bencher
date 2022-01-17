use url::Url;
use yew::{function_component, html, use_state, Callback, Html, Properties};

use rollback::account::{Account, AccountKind, Accounts};
use rollback::institution::{Institution, Institutions};

#[derive(Properties, PartialEq)]
pub struct InstitutionsListProps {
    pub institutions: Institutions,
}

#[function_component(InstitutionsList)]
pub fn institutions_list(props: &InstitutionsListProps) -> Html {
    let add_institution = use_state(|| false);

    let onclick = {
        let add_institution = add_institution.clone();
        Callback::from(move |_| add_institution.set(!(*add_institution)))
    };

    html! {
        <div>
        <div id="institutions">
            {
                props.institutions.iter().map(|(institution, _accounts)| {
                    html!{<div key={ institution.name() }>{ institution.name() }</div>}
                }).collect::<Html>()
            }
        </div>
        <button class="button" onclick={onclick} >{ "+Add" }</button>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct InstitutionCardProps {
    pub institution: Institution,
}

#[function_component(InstitutionCard)]
pub fn institution_card(props: &InstitutionCardProps) -> Html {
    html! {
        <div class="card">
            <div class="card-content">
                <div class="content">
                    <button class="button">{ "+Add" }</button>
                </div>
            </div>
        </div>
    }
}
