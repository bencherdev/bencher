use sycamore::prelude::*;

use url::Url;

use rollback::institution::{Institution, InstitutionAccounts, Institutions};

fn main() {
    sycamore::render(|| {
        let institutions = Signal::new({
            let mut instatutions = Institutions::new();
            let institution = Institution::new("a".into(), Url::parse("http://goop.com").unwrap());
            instatutions.insert(institution.clone(), InstitutionAccounts::new(institution));
            instatutions
        });

        let institutions_vec = create_memo(
            cloned!(institutions => move || institutions.get().values().cloned().collect::<Vec<InstitutionAccounts>>()),
        );

        view! {
            ul {
                Keyed(KeyedProps {
                    iterable: institutions_vec,
                    template: |ia| view! {
                        li { (ia) }
                    },
                    key: |ia| (ia.clone()) ,
                })
            }
        }
    });
}
