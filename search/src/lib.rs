// https://github.com/tinysearch/tinysearch/blob/master/engine/src/lib.rs
use once_cell::sync::Lazy;
use xorf::{HashProxy, Xor8};

use std::cmp::Reverse;
use std::collections::hash_map::DefaultHasher;

use tinysearch_shared::{Filters, PostId, Score, Storage};
pub type Filter = HashProxy<String, DefaultHasher, Xor8>;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

const TITLE_WEIGHT: usize = 3;

static FILTERS: Lazy<Filters> = Lazy::new(|| {
    let bytes = include_bytes!("../../data/ticker/nyse.json");
    Storage::from_bytes(bytes).unwrap().filters
});

// Wrapper around filter score, that also scores the post title
// Post title score has a higher weight than post body
fn score(title: &String, search_terms: &Vec<String>, filter: &Filter) -> usize {
    let title_terms: Vec<String> = tokenize(&title);
    let title_score: usize = search_terms
        .iter()
        .filter(|term| title_terms.contains(&term))
        .count();
    TITLE_WEIGHT * title_score + filter.score(search_terms)
}

fn tokenize(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split_whitespace()
        .filter(|&t| !t.trim().is_empty())
        .map(String::from)
        .collect()
}

pub fn search<'p>(query: String, num_results: usize) -> Vec<&'p PostId> {
    let search_terms: Vec<String> = tokenize(&query);

    let mut matches: Vec<(&PostId, usize)> = FILTERS
        .iter()
        .map(|(post_id, filter)| (post_id, score(&post_id.0, &search_terms, &filter)))
        .filter(|(_post_id, score)| *score > 0)
        .collect();

    matches.sort_by_key(|k| Reverse(k.1));

    matches.into_iter().take(num_results).map(|p| p.0).collect()
}
