use url::Url;
use yew::{function_component, html, use_state, Callback, Html, Properties};

use rollback::account::{Account, AccountKind, Accounts};
use rollback::holding::Holding;
use rollback::institution::{Institution, Institutions};
use rollback::portfolio::{Portfolio, User};
use rollback::ticker::TickerSymbols;
use rollback::total::Total;
use rollback::transaction::{Transaction, TransactionKind, Transactions};

mod account;
mod accounts;
mod institution;
mod institutions;

use institutions::InstitutionsList;

#[function_component(Index)]
fn index() -> Html {
    let portfolio = use_state(get_portfolio);

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
            <nav class="navbar" role="navigation" aria-label="main navigation">
                <div class="navbar-brand">
                    <div class="navbar-item">
                        {portfolio.to_string()}
                    </div>
                </div>

                <div class="navbar-end">
                    <div class="navbar-item">
                        {portfolio.total()}
                    </div>
                </div>
            </nav>

            <section class="section">
                <div class="container">
                    <InstitutionsList institutions={portfolio.institutions().clone()} />
                </div>
            </section>
        </div>
    }
}

fn main() {
    yew::start_app::<Index>();
}

fn get_portfolio() -> Portfolio {
    let user = User::new("Bob".into(), "Saget".into());
    let mut portfolio = Portfolio::new(user);

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
    let ticker = tickers.first().unwrap();
    account
        .holdings_mut()
        .insert(ticker.clone(), Holding::new(ticker.clone()));
    account
        .holdings_mut()
        .get_mut(ticker)
        .unwrap()
        .transactions_mut()
        .add(Transaction::new(TransactionKind::Buy, 1000));

    accounts.insert(id.into(), account);
    portfolio
        .institutions_mut()
        .insert(institution.clone(), accounts);
    let institution = Institution::new(
        "Vangaurd".into(),
        Url::parse("https://vanguard.com").unwrap(),
    );
    portfolio
        .institutions_mut()
        .insert(institution.clone(), Accounts::new());

    // Schwab
    let institution = Institution::new(
        "Charles Schwab".into(),
        Url::parse("https://schwab.com").unwrap(),
    );
    portfolio
        .institutions_mut()
        .insert(institution.clone(), Accounts::new());
    portfolio
}
