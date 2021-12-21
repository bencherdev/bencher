use std::collections::HashMap;
use std::convert::TryFrom;
use url::Url;

use tinysearch_shared::PostId;

use search::TICKER;

pub struct TickerSymbols;

impl TickerSymbols {
    pub fn search(query: &str, num_results: usize) -> Vec<TickerSymbol> {
        let ticker = TICKER.search(query, num_results);
        let mut ticker_symbols = Vec::new();
        for t in ticker {
            if let Ok(t) = TickerSymbol::try_from(t) {
                ticker_symbols.push(t);
            }
        }
        ticker_symbols
    }
}

pub struct TickerSymbol {
    name: String,
    symbol: String,
}

impl TryFrom<&PostId> for TickerSymbol {
    type Error = anyhow::Error;

    fn try_from(value: &PostId) -> anyhow::Result<Self> {
        let (title, url) = value;
        let url = Url::parse(url)?;
        let mut symbol = None;
        for (key, value) in url.query_pairs() {
            if key == "symbol" {
                symbol = Some(value.into())
            }
        }
        if let Some(symbol) = symbol {
            Ok(Self {
                name: title.into(),
                symbol,
            })
        } else {
            Err(anyhow::anyhow!("Failed to find symbol"))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn ticker_symbols_search_goop() {
        assert_eq!(TickerSymbols::search("GOOP", 5).len(), 0);
    }

    #[test]
    fn ticker_symbols_search_vtsax() {
        assert_eq!(TickerSymbols::search("VTSAX", 5).len(), 1);
    }
}
