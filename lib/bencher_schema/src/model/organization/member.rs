use bencher_json::{
    organization::member::OrganizationRole, DateTime, Email, JsonMember, Slug, UserName, UserUuid,
};

#[derive(diesel::Queryable)]
pub struct QueryMember {
    pub uuid: UserUuid,
    pub name: UserName,
    pub slug: Slug,
    pub email: Email,
    pub role: OrganizationRole,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryMember {
    pub fn into_json(self) -> JsonMember {
        let Self {
            uuid,
            name,
            slug,
            email,
            role,
            created,
            modified,
        } = self;
        JsonMember {
            uuid,
            name,
            slug,
            email,
            role,
            created,
            modified,
        }
    }
}
