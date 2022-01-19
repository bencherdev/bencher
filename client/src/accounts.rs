use url::Url;
use yew::{function_component, html, use_state, Callback, Html, Properties};

use rollback::account::{Account, AccountKind, Accounts};
use rollback::institution::{Institution, Institutions};
use rollback::total::Total;

use crate::account::AccountCard;

#[derive(Properties, PartialEq)]
pub struct AccountsListProps {
    pub accounts: Accounts,
}

#[function_component(AccountsList)]
pub fn accounts_list(props: &AccountsListProps) -> Html {
    let add_account = use_state(|| false);

    let onclick = {
        let add_account = add_account.clone();
        Callback::from(move |_| add_account.set(!(*add_account)))
    };

    html! {
        <div>
            <div id="accounts">
                {
                    props.accounts.iter().map(|(id, account)| {
                        html!{
                            <div>
                                <AccountCard
                                    key={id.as_ref()}
                                    account={account.clone()}
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
                        { *add_account }
                    </p>
                </div>
            </div>
        </div>
    }
}
