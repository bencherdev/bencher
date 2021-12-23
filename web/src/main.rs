use sycamore::prelude::*;

use url::Url;

use rollback::account::Accounts;
use rollback::institution::{Institution, Institutions};

mod institution;
use institution::CoInstitutions;

fn main() {
    sycamore::render(|| {
        let institutions = Signal::new(get_institutions());

        let institutions_vec = create_memo(
            cloned!(institutions => move || institutions.get().iter().map(|(inst, accs)| (inst.clone(), accs.clone())).collect::<Vec<(Institution, Accounts)>>()),
        );

        view! {
            CoInstitutions(institutions_vec)
        }
    });
}

fn get_institutions() -> Institutions {
    let mut instatutions = Institutions::new();
    let institution = Institution::new(
        "Fidelity".into(),
        Url::parse("https://fidelity.com").unwrap(),
    );
    instatutions.insert(institution.clone(), Accounts::new());
    let institution = Institution::new(
        "Vangaurd".into(),
        Url::parse("https://vanguard.com").unwrap(),
    );
    instatutions.insert(institution.clone(), Accounts::new());
    let institution = Institution::new(
        "Charles Schwab".into(),
        Url::parse("https://schwab.com").unwrap(),
    );
    instatutions.insert(institution.clone(), Accounts::new());
    instatutions
}
