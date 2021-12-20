use std::collections::HashMap;

pub struct TickerSymbols {
    symbols: HashMap<String, TickerSymbol>,
}

impl TickerSymbols {
    fn load() -> Self {
        // TODO read in data from ./data/ticker using serde json
        // Note that these files should be in a `tinysearch` compliant format
        Self {
            symbols: HashMap::new(),
        }
    }

    fn search(&self, query: &str) -> Option<&TickerSymbol> {
        // TODO use a `tinysearch` for the actual operation
        self.symbols.get(query)
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
