#[derive(Clone, Hash, Eq, PartialEq)]
pub struct Transactions {
    history: Vec<Transaction>,
    quantity: u64,
}

impl Transactions {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            quantity: 0,
        }
    }

    pub fn quantity(&self) -> u64 {
        self.quantity
    }

    pub fn history(&self) -> &Vec<Transaction> {
        &self.history
    }

    pub fn add(&mut self, transaction: Transaction) {
        match &transaction.kind {
            TransactionKind::Buy => self.quantity += transaction.quantity,
            TransactionKind::Sell => self.quantity -= transaction.quantity,
        }
        self.history.push(transaction);
        // self.history.sort()
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.history.len() {
            let transaction = self.history.remove(index);
            match &transaction.kind {
                TransactionKind::Buy => self.quantity -= transaction.quantity,
                TransactionKind::Sell => self.quantity += transaction.quantity,
            }
        }
    }
}

// TODO implement sort and find

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct Transaction {
    kind: TransactionKind,
    quantity: u64,
}

impl Transaction {
    pub fn new(kind: TransactionKind, quantity: u64) -> Self {
        Self { kind, quantity }
    }
}

// TODO make this much more advanced

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum TransactionKind {
    Buy,
    Sell,
}
