use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Cell {
    Text(TextCell),
    Number(NumberCell),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextCell {
    value: String,
}

impl TextCell {
    pub fn new() -> TextCell {
        TextCell {
            value: String::new(),
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn set_value(&mut self, value: &str) {
        self.value = value.to_owned();
    }
}

impl From<Cell> for TextCell {
    fn from(value: Cell) -> Self {
        match value {
            Cell::Text(text) => text,
            Cell::Number(number) => TextCell::from(number),
        }
    }
}

impl From<NumberCell> for TextCell {
    fn from(value: NumberCell) -> Self {
        TextCell {
            value: value.value().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumberCell {
    value: f64,
}

impl NumberCell {
    pub fn new() -> NumberCell {
        NumberCell { value: 0.0 }
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn set_value(&mut self, value: f64) {
        self.value = value;
    }
}

impl From<Cell> for NumberCell {
    fn from(value: Cell) -> Self {
        match value {
            Cell::Text(text) => NumberCell::from(text),
            Cell::Number(number) => number,
        }
    }
}

impl From<TextCell> for NumberCell {
    fn from(value: TextCell) -> Self {
        NumberCell {
            value: match value.value().parse::<f64>() {
                Ok(number) => number,
                Err(_) => 0.0,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    pub fn test_text_cell_new() {
        assert_eq!("".to_owned(), TextCell::new().value());
    }

    #[test]
    pub fn test_number_cell_new() {
        assert_eq!(0.0, NumberCell::new().value());
    }

    #[test]
    pub fn test_text_cell_set() {
        let mut cell = TextCell::new();
        let empty = "".to_owned();
        assert_eq!(empty, cell.value());

        let number_cell = NumberCell::from(cell.clone());
        assert_eq!(0.0, number_cell.value());

        let first = "Saul";
        cell.set_value(first);
        assert!(first != empty);
        assert_eq!(first, cell.value());

        let number_cell = NumberCell::from(cell.clone());
        assert_eq!(0.0, number_cell.value());

        let last = "Goodman";
        cell.set_value(last);
        assert!(last != empty);
        assert!(last != first);
        assert_eq!(last, cell.value());

        let number_cell = NumberCell::from(cell.clone());
        assert_eq!(0.0, number_cell.value());

        let age_number = 42;
        let age = 42.to_string();
        cell.set_value(&age);
        assert!(age != empty);
        assert!(age != first);
        assert!(age != last);
        assert_eq!(age, cell.value());

        let number_cell = NumberCell::from(cell);
        assert_eq!(age_number as f64, number_cell.value());
    }

    #[test]
    pub fn test_number_cell_set() {
        let mut cell = NumberCell::new();
        let zero = 0.0;
        assert_eq!(zero, cell.value());

        let text_cell = TextCell::from(cell.clone());
        assert_eq!(zero.to_string(), text_cell.value());

        let bb_age = 48.0;
        cell.set_value(bb_age);
        assert!(bb_age != zero);
        assert_eq!(bb_age, cell.value());

        let text_cell = TextCell::from(cell.clone());
        assert_eq!(bb_age.to_string(), text_cell.value());

        let bcs_age = 41.0;
        cell.set_value(bcs_age);
        assert!(bcs_age != zero);
        assert!(bcs_age != bb_age);
        assert_eq!(bcs_age, cell.value());

        let text_cell = TextCell::from(cell.clone());
        assert_eq!(bcs_age.to_string(), text_cell.value());

        let gene_age = 50.0;
        cell.set_value(gene_age);
        assert!(gene_age != zero);
        assert!(gene_age != bb_age);
        assert!(gene_age != bcs_age);
        assert_eq!(gene_age, cell.value());

        let text_cell = TextCell::from(cell);
        assert_eq!(gene_age.to_string(), text_cell.value());
    }
}
