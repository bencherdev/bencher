use url::Url;
use yew::{function_component, html, use_state, Callback, Html};

use rollback::account::{Account, AccountKind, Accounts};
use rollback::institution::{Institution, Institutions};
use rollback::ticker::TickerSymbols;

#[function_component(UseState)]
fn state() -> Html {
    let counter = use_state(|| 0);
    let institutions = use_state(get_institutions);

    let onclick = {
        let counter = counter.clone();
        Callback::from(move |_| counter.set(*counter + 1))
    };

    html! {
        <div>
            <button class="button" {onclick}>{ "Increment value" }</button>
            <p>
                <b>{ "Current value: " }</b>
                { *counter }
            </p>
            <div id="introductions">
            {
                institutions.iter().map(|(institution, _accounts)| {
                    html!{<div key={ institution.name() }>{ institution.name() }</div>}
                }).collect::<Html>()
            }
        </div>
        </div>
    }
}

fn main() {
    yew::start_app::<UseState>();
}

fn get_institutions() -> Institutions {
    let mut institutions = Institutions::new();

    // Fidelity
    let institution = Institution::new(
        "Fidelity".into(),
        Url::parse("https://fidelity.com").unwrap(),
    );
    let mut accounts = Accounts::new();

    // Vanguard
    let id = "abc";
    let mut account = Account::new(id.into(), AccountKind::Brokerage);

    let tickers = TickerSymbols::search("vtsax", 1);
    account.add_investment(tickers.first().unwrap().clone(), 10);

    accounts.insert(id.into(), account);
    institutions.insert(institution.clone(), accounts);
    let institution = Institution::new(
        "Vangaurd".into(),
        Url::parse("https://vanguard.com").unwrap(),
    );
    institutions.insert(institution.clone(), Accounts::new());

    // Schwab
    let institution = Institution::new(
        "Charles Schwab".into(),
        Url::parse("https://schwab.com").unwrap(),
    );
    institutions.insert(institution.clone(), Accounts::new());
    institutions
}
