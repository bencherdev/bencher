use im::hashmap::HashMap;

use crate::studio::table::cell::Cell;
use crate::studio::uuid::Uuid;

pub type Row = HashMap<Uuid, Cell>;
