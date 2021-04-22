use std::fmt;

use serde::{Deserialize, Serialize};

use crate::studio::table::cell::Cell;
use crate::studio::uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    id: Uuid,
    name: Option<String>,
    data_type: DataType,
}

impl Header {
    pub fn new(name: Option<String>, data_type: DataType) -> Header {
        Header {
            id: Uuid::new(),
            name: name,
            data_type: data_type,
        }
    }

    pub fn id(&self) -> &Uuid {
        &self.id
    }

    pub fn name(&self) -> &Option<String> {
        &self.name
    }

    pub fn set_name(&mut self, name: Option<String>) {
        self.name = name;
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

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                DataType::Text => "String",
                DataType::Number => "f64",
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    const DEFAULT_HEADER_NAME: Option<String> = None;

    #[test]
    pub fn test_header_new() {
        let default_data_type = DataType::Text;
        let default_header = Header::new(DEFAULT_HEADER_NAME, default_data_type.clone());

        assert_eq!(DEFAULT_HEADER_NAME, *default_header.name());
        match default_header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let first_name = Some("First".to_owned());
        let first_data_type = DataType::Text;
        let first_header = Header::new(first_name.clone(), first_data_type.clone());

        assert_eq!(first_name, *first_header.name());
        match first_header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }
        assert!(*first_header.id() != *default_header.id());

        let last_name = Some("Last".to_owned());
        let last_data_type = DataType::Text;
        let last_header = Header::new(last_name.clone(), last_data_type.clone());

        assert_eq!(last_name, *last_header.name());
        match last_header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }
        assert!(*last_header.id() != *default_header.id());
        assert!(*last_header.id() != *first_header.id());

        let age_name = Some("Age".to_owned());
        let age_data_type = DataType::Number;
        let age_header = Header::new(age_name.clone(), age_data_type.clone());

        assert_eq!(age_name, *age_header.name());
        match age_header.data_type() {
            DataType::Number => {}
            _ => panic!("test"),
        }
        assert!(*age_header.id() != *default_header.id());
        assert!(*age_header.id() != *first_header.id());
        assert!(*age_header.id() != *last_header.id());

        let no_name_age_header = Header::new(DEFAULT_HEADER_NAME, age_data_type.clone());

        assert_eq!(DEFAULT_HEADER_NAME, *no_name_age_header.name());
        match no_name_age_header.data_type() {
            DataType::Number => {}
            _ => panic!("test"),
        }
        assert!(*no_name_age_header.id() != *default_header.id());
        assert!(*no_name_age_header.id() != *first_header.id());
        assert!(*no_name_age_header.id() != *last_header.id());
        assert!(*no_name_age_header.id() != *age_header.id());
    }

    #[test]
    pub fn test_header_set() {
        let default_data_type = DataType::Text;
        let mut header = Header::new(DEFAULT_HEADER_NAME, default_data_type.clone());
        let id = header.id().clone();

        assert_eq!(DEFAULT_HEADER_NAME, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let first_name = Some("First".to_owned());
        header.set_name(first_name.clone());
        assert_eq!(first_name, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }
        assert_eq!(id, *header.id());

        let last_name = Some("Last".to_owned());
        header.set_name(last_name.clone());
        assert_eq!(last_name, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }
        assert_eq!(id, *header.id());

        let last_data_type = DataType::Text;
        assert_eq!(last_name, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }
        assert_eq!(id, *header.id());

        let age_name = Some("Age".to_owned());
        header.set_name(age_name.clone());
        assert_eq!(age_name, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }
        assert_eq!(id, *header.id());

        let age_data_type = DataType::Number;
        header.set_data_type(age_data_type);
        assert_eq!(age_name, *header.name());
        match header.data_type() {
            DataType::Number => {}
            _ => panic!("test"),
        }
        assert_eq!(id, *header.id());

        header.set_name(DEFAULT_HEADER_NAME);
        assert_eq!(DEFAULT_HEADER_NAME, *header.name());
        match header.data_type() {
            DataType::Number => {}
            _ => panic!("test"),
        }
        assert_eq!(id, *header.id());
    }
}
