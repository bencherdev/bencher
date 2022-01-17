use url::Url;
use yew::{function_component, html, use_state, Callback, Html, Properties};

use rollback::account::{Account, AccountKind, Accounts};
use rollback::institution::{Institution, Institutions};
use rollback::total::Total;

#[derive(Properties, PartialEq)]
pub struct InstitutionCardProps {
    pub institution: Institution,
    pub accounts: Accounts,
}

#[function_component(InstitutionCard)]
pub fn institution_card(props: &InstitutionCardProps) -> Html {
    html! {
        <div class="card">
            <div class="card-header">
                <p class="card-header-title">
                    { props.institution.to_string() }
                </p>

                <p class="card-header-icon">
                    { props.accounts.total() }
                </p>

                <button class="card-header-icon" aria-label="See Accounts">
                    <span class="icon">
                        <i class="fas fa-angle-down" aria-hidden="true"></i>
                    </span>
                </button>
            </div>

            <div class="card-content">
                { "TODO Accounts List"}
            </div>

            <footer class="card-footer">
                <button class="card-footer-item">{"Edit"}</button>
            </footer>
        </div>
    }
}
