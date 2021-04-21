use seed::{prelude::*, *};

pub fn view<Ms>() -> Node<Ms> {
    div![
        attrs![At::Class => "columns"],
        div![
            attrs![At::Class => "column is-half"],
            div![
                attrs![At::Class => "content"],
                div![
                    attrs![At::Class => "table-container"],
                    table![
                        attrs![At::Class => "table is-bordered is-hoverable is-narrow"],
                        thead![tr![th!["Names"]],],
                        thead![tr![th!["First"], th!["Last"],],],
                        tbody![tr![td!["Saul"], td!["Goodman"],],],
                    ],
                ],
            ],
        ]
    ]
}
