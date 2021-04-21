use serde::{Deserialize, Serialize};

use crate::studio::table::cell::Cell;
use crate::studio::table::table::TableMsg;
use crate::studio::uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    id: Uuid,
    name: String,
    data_type: DataType,
}

impl Header {
    pub fn new(name: &str, data_type: DataType) -> Header {
        Header {
            id: Uuid::new(),
            name: name.to_owned(),
            data_type: data_type,
        }
    }

    pub fn id(&self) -> &Uuid {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_owned();
    }

    pub fn data_type(&self) -> &DataType {
        &self.data_type
    }

    pub fn set_data_type(&mut self, data_type: DataType) {
        self.data_type = data_type;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataType {
    Text,
    Number,
}

impl DataType {
    // TODO move over to a drop box returned via Node<TableMsg>
    pub fn to_html(&self) -> String {
        match self {
            DataType::Text => "Text".to_owned(),
            DataType::Number => "Number".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    pub fn test_header_new() {
        let first_name = "First";
        let first_data_type = DataType::Text;
        let first_header = Header::new(first_name, first_data_type.clone());

        assert_eq!(first_name, first_header.name());
        match first_header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let last_name = "Last";
        let last_data_type = DataType::Text;
        let last_header = Header::new(last_name, last_data_type.clone());

        assert_eq!(last_name, last_header.name());
        match last_header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }
        assert!(*first_header.id() != *last_header.id());

        let age_name = "Age";
        let age_data_type = DataType::Number;
        let age_header = Header::new(age_name, age_data_type.clone());

        assert_eq!(age_name, age_header.name());
        match age_header.data_type() {
            DataType::Number => {}
            _ => panic!("test"),
        }
        assert!(*age_header.id() != *first_header.id());
        assert!(*age_header.id() != *last_header.id());
    }

    #[test]
    pub fn test_header_set() {
        let first_name = "First";
        let first_data_type = DataType::Text;
        let mut header = Header::new(first_name, first_data_type.clone());
        let id = header.id().clone();

        assert_eq!(first_name, header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }
        assert_eq!(id, *header.id());

        let last_name = "Last";
        header.set_name(last_name);
        assert_eq!(last_name, header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }
        assert_eq!(id, *header.id());

        let last_data_type = DataType::Text;
        assert_eq!(last_name, header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }
        assert_eq!(id, *header.id());

        let age_name = "Age";
        header.set_name(age_name);
        assert_eq!(age_name, header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }
        assert_eq!(id, *header.id());

        let age_data_type = DataType::Number;
        header.set_data_type(age_data_type);
        assert_eq!(age_name, header.name());
        match header.data_type() {
            DataType::Number => {}
            _ => panic!("test"),
        }
        assert_eq!(id, *header.id());
    }
}
