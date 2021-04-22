use english_numbers;

pub fn nameless(index: usize) -> String {
    english_numbers::convert(
        (index + 1) as i64,
        english_numbers::Formatting {
            title_case: true,
            spaces: true,
            conjunctions: false,
            commas: false,
            dashes: false,
        },
    )
}
