use sycamore::prelude::*;

use url::Url;

use rollback::account::{Account, AccountKind, Accounts};
use rollback::institution::{Institution, Institutions};
use rollback::ticker::TickerSymbols;

mod account;
mod institution;
mod investment;
use institution::CoInstitutions;

fn main() {
    sycamore::render(|| {
        let counter = Signal::new(0);
        let add_institution = Signal::new(false);

        let institutions = Signal::new(get_institutions());

        let institutions_vec = create_memo(
            cloned!(institutions => move || institutions.get().iter().map(|(inst, accs)| (inst.clone(), accs.clone())).collect::<Vec<(Institution, Accounts)>>()),
        );

        let increment = cloned!((counter) => move |_| counter.set(*counter.get() + 1));

        let toggle_add_institution =
            cloned!((add_institution) => move |_| add_institution.set(!(*add_institution.get())));

        view! {
            CoInstitutions(institutions_vec)

            p {
                (*counter.get())
            }

            button(class="increment", on:click=increment) {
                "Increment"
            }

            p {
                (*add_institution.get())
            }

            button(class="add-investment", on:click=toggle_add_institution) {
                "Add Institution"
            }
        }
    });
}

fn get_institutions() -> Institutions {
    let mut instatutions = Institutions::new();

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
    instatutions.insert(institution.clone(), accounts);
    let institution = Institution::new(
        "Vangaurd".into(),
        Url::parse("https://vanguard.com").unwrap(),
    );
    instatutions.insert(institution.clone(), Accounts::new());

    // Schwab
    let institution = Institution::new(
        "Charles Schwab".into(),
        Url::parse("https://schwab.com").unwrap(),
    );
    instatutions.insert(institution.clone(), Accounts::new());
    instatutions
}
