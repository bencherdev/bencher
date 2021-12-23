use sycamore::prelude::*;

use url::Url;

use rollback::institution::{Institution, InstitutionAccounts, Institutions};

fn main() {
    sycamore::render(|| {
        let institutions = Signal::new(get_institutions());

        let institutions_vec = create_memo(
            cloned!(institutions => move || institutions.get().keys().cloned().collect::<Vec<Institution>>()),
        );

        view! {
            ul {
                Keyed(KeyedProps {
                    iterable: institutions_vec,
                    template: |i| view! {
                        li { (i) }
                    },
                    key: |i| (i.clone()) ,
                })
            }
        }
    });
}

fn get_institutions() -> Institutions {
    let mut instatutions = Institutions::new();
    let institution = Institution::new(
        "Fidelity".into(),
        Url::parse("https://fidelity.com").unwrap(),
    );
    instatutions.insert(institution.clone(), InstitutionAccounts::new());
    let institution = Institution::new(
        "Vangaurd".into(),
        Url::parse("https://vanguard.com").unwrap(),
    );
    instatutions.insert(institution.clone(), InstitutionAccounts::new());
    let institution = Institution::new(
        "Charles Schwab".into(),
        Url::parse("https://schwab.com").unwrap(),
    );
    instatutions.insert(institution.clone(), InstitutionAccounts::new());
    instatutions
}
