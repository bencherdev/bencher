macro_rules! auth_error {
    ($message:expr) => {
        || {
            tracing::info!($message);
            crate::error::ApiError::Auth($message.into())
        }
    };
}

pub(crate) use auth_error;

macro_rules! map_auth_error {
    ($message:expr) => {
        |e| {
            tracing::info!("{}: {}", $message, e);
            crate::error::ApiError::Auth($message.into())
        }
    };
}

pub(crate) use map_auth_error;

macro_rules! query_roles {
    ($conn:ident, $user_id:expr, $table:ident, $user_id_field:ident, $field:ident, $role_field:ident, $msg:expr) => {{
        let roles = schema::$table::table
            .filter(schema::$table::$user_id_field.eq($user_id))
            .order(schema::$table::$field)
            .select((schema::$table::$field, schema::$table::$role_field))
            .load::<(i32, String)>($conn)
            .map_err(map_auth_error!(INVALID_JWT))?;

        let ids = roles.iter().map(|(id, _)| *id).collect();
        let roles = roles
            .into_iter()
            .filter_map(|(id, role)| match role.parse() {
                Ok(role) => Some((id.to_string(), role)),
                Err(e) => {
                    tracing::error!($msg, role, e);
                    debug_assert!(false, $msg, role, e);
                    None
                },
            })
            .collect();

        Ok((ids, roles))
    }};
}

pub(crate) use query_roles;

macro_rules! org_roles_map {
    ($conn:ident, $user_id:expr) => {
        super::macros::query_roles!(
            $conn,
            $user_id,
            organization_role,
            user_id,
            organization_id,
            role,
            "Failed to parse organization role {}: {}"
        )
    };
}

pub(crate) use org_roles_map;

macro_rules! proj_roles_map {
    ($conn:ident, $user_id:expr) => {
        super::macros::query_roles!(
            $conn,
            $user_id,
            project_role,
            user_id,
            project_id,
            role,
            "Failed to parse project role {}: {}"
        )
    };
}

pub(crate) use proj_roles_map;
