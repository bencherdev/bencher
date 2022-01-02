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

        let institutions = Signal::new(get_institutions());

        let add_institution = cloned!((institutions) => move |_:()| {
            // Added
            let institution = Institution::new(
                "Added".into(),
                Url::parse("https://add.com").unwrap(),
            );
            let mut institutions = institutions.get();
            let mut institutions = std::rc::Rc::get_mut(&mut institutions).unwrap();
            institutions.insert(institution, Accounts::new());
            // institutions.set();
        });

        let institutions_vec = create_memo(
            cloned!(institutions => move || institutions.get().iter().map(|(inst, accs)| (inst.clone(), accs.clone())).collect::<Vec<(Institution, Accounts)>>()),
        );

        let increment = cloned!((counter) => move |_| counter.set(*counter.get() + 1));

        view! {
            CoInstitutions(institutions_vec)

            p {
                (*counter.get())
            }

            button(class="increment", on:click=increment) {
                "Increment"
            }

            AddInstitutionButton(add_institution)
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

#[component(AddInstitutionButton<G>)]
pub fn add_institution_button<F>(_a: F) -> View<G>
where
    F: Fn(()) -> (),
{
    let button_state = Signal::new(false);

    let toggle_button_state =
        cloned!((button_state) => move |_| button_state.set(!(*button_state.get())));

    view! {
        p {
            (*button_state.get())
        }

        button(class="add-investment", on:click=toggle_button_state) {
            "Add Institution"
        }
    }
}
