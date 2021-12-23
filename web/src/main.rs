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
