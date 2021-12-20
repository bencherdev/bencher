use std::collections::HashMap;
use std::convert::TryFrom;

use tinysearch_shared::PostId;

use search::TICKER;

pub struct TickerSymbols {
    symbols: HashMap<String, TickerSymbol>,
}

impl TickerSymbols {
    fn search(&self, query: &str, num_results: usize) -> Vec<TickerSymbol> {
        // TODO use a `tinysearch` for the actual operation
        let ticker = TICKER.search(query, num_results);
        Vec::new()
    }
}

impl TryFrom<&PostId> for TickerSymbols {
    type Error = &'static str;

    fn try_from(value: &PostId) -> Result<Self, Self::Error> {
        Err("")
    }
}

pub struct TickerSymbol {
    stock_exchange: StockExchange,
    symbol: String,
}

pub enum StockExchange {
    NASDAQ,
    NYSE,
}

mod test {
    use super::*;

    #[test]
    fn ticker_symbols_load() {
        let ticker_symbols = TickerSymbols::load();
        assert_eq!(ticker_symbols.symbols.len(), 0);
    }

    #[test]
    fn ticker_symbols_search() {
        let ticker_symbols = TickerSymbols::load();
        assert!(ticker_symbols.search("GOOP").is_none());
    }
}
