// https://github.com/tinysearch/tinysearch/blob/master/engine/src/lib.rs
use once_cell::sync::Lazy;
use xorf::{HashProxy, Xor8};

use anyhow::Result;

use std::cmp::Reverse;
use std::collections::hash_map::DefaultHasher;

use tinysearch_shared::{Filters, PostId, Score};
pub type Filter = HashProxy<String, DefaultHasher, Xor8>;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

const TITLE_WEIGHT: usize = 3;

mod index;
mod storage;

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

pub struct Search(pub Lazy<Filters>);

impl Search {
    pub fn load(raw: &str) -> Result<Filters> {
        storage::load(raw.into())
    }

    pub fn search<'p>(&'p self, query: &str, num_results: usize) -> Vec<&'p PostId> {
        let search_terms: Vec<String> = tokenize(query);

        let mut matches: Vec<(&PostId, usize)> = self
            .0
            .iter()
            .map(|(post_id, filter)| (post_id, score(&post_id.0, &search_terms, &filter)))
            .filter(|(_post_id, score)| *score > 0)
            .collect();

        matches.sort_by_key(|k| Reverse(k.1));

        matches.into_iter().take(num_results).map(|p| p.0).collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    static TICKER: Search = Search(Lazy::new(|| {
        Search::load(include_str!("../../data/ticker.json")).unwrap()
    }));

    #[test]
    fn ticker_symbols_search_goop() {
        assert_eq!(TICKER.search("GOOP", 5).len(), 0);
    }

    #[test]
    fn ticker_symbols_search_vtsax() {
        assert_eq!(TICKER.search("VTSAX", 5).len(), 1);
    }
}
