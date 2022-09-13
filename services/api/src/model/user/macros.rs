macro_rules! query_roles {
    ($conn:ident, $user_id:expr, $table:ident, $user_id_field:ident, $field:ident, $select:expr, $load:ty) => {
        schema::$table::table
            .filter(schema::$table::$user_id_field.eq($user_id))
            .order(schema::$table::$field)
            .select($select)
            .load::<$load>($conn)
            .map_err(map_auth_error!(INVALID_JWT))
    };
}

macro_rules! filter_roles_map {
    ($query:ident, $msg:expr) => {
        $query
            .into_iter()
            .filter_map(|(id, role)| match role.parse() {
                Ok(role) => Some((id.to_string(), role)),
                Err(e) => {
                    tracing::error!($msg, role, e);
                    debug_assert!(false, $msg, role, e);
                    None
                },
            })
            .collect()
    };
}

macro_rules! roles_map {
    ($conn:ident, $user_id:expr, $table:ident, $user_id_field:ident, $field:ident, $role_field:ident, $msg:expr) => {{
        let query = query_roles!(
            $conn,
            $user_id,
            $table,
            $user_id_field,
            $field,
            (schema::$table::$field, schema::$table::$role_field),
            (i32, String)
        )?;
        Ok(filter_roles_map!(query, $msg))
    }};
}

pub(crate) use roles_map;

macro_rules! roles_vec {
    ($conn:ident, $user_id:expr, $table:ident, $user_id_field:ident, $field:ident, $role_field:ident) => {
        query_roles!(
            $conn,
            $user_id,
            $table,
            $user_id_field,
            $field,
            schema::$table::$field,
            i32
        )?
        .into_iter()
        .filter_map(|id| {})
        .collect()
    };
}

pub(crate) use roles_vec;
