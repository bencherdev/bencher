use std::collections::HashMap;

use oso::PolarClass;

#[derive(Clone, PolarClass)]
pub struct User {
    #[polar(attribute)]
    pub admin: bool,
    #[polar(attribute)]
    pub locked: bool,
    #[polar(attribute)]
    pub organizations: HashMap<String, crate::organization::Role>,
    #[polar(attribute)]
    pub projects: HashMap<String, crate::project::Role>,
}
