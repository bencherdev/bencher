use url::Url;
use yew::{function_component, html, use_state, Callback, Html, Properties};

use rollback::account::{Account, AccountKind, Accounts};
use rollback::institution::{Institution, Institutions};
use rollback::ticker::TickerSymbols;

mod institutions;

use institutions::InstitutionsList;

#[function_component(Index)]
fn index() -> Html {
    let institutions = use_state(get_institutions);

    // let add_institution = {
    //     let institutions = institutions.clone();
    //     Callback::from(move |_| (*institutions).insert())
    // };

    // let onclick = {
    //     let counter = counter.clone();
    //     Callback::from(move |_| counter.set(*counter + 1))
    // };

    html! {
        <div>
            <section class="section">
                <div class="container">
                    <InstitutionsList institutions={(*institutions).clone()} />
                </div>
            </section>
        </div>
    }
}

fn main() {
    yew::start_app::<Index>();
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
