use url::Url;
use yew::{function_component, html, use_state, Callback, Html, Properties};

use rollback::account::{Account, AccountKind, Accounts};
use rollback::institution::{Institution, Institutions};
use rollback::total::Total;

use crate::institution::InstitutionCard;

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
                    props.institutions.iter().map(|(institution, accounts)| {
                        html!{
                            <div>
                                <InstitutionCard
                                    key={institution.name()}
                                    institution={institution.clone()}
                                    accounts={accounts.clone()}
                                />
                                <br/>
                            </div>
                        }
                    }).collect::<Html>()
                }
            </div>

            <div class="card">
                <div class="card-content">
                    <div class="content">
                        <button class="button is-fullwidth" onclick={onclick}>{ "+Add" }</button>
                    </div>

                    <p class="card-header-icon">
                        { *add_institution }
                    </p>
                </div>
            </div>
        </div>
    }
}
