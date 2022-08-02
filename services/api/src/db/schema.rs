table! {
    adapter (id) {
        id -> Integer,
        uuid -> Text,
        name -> Text,
    }
}

table! {
    branch (id) {
        id -> Integer,
        uuid -> Text,
        project_id -> Integer,
        name -> Text,
        slug -> Text,
    }
}

table! {
    project (id) {
        id -> Integer,
        uuid -> Text,
        owner_id -> Integer,
        name -> Text,
        slug -> Text,
        description -> Nullable<Text>,
        url -> Nullable<Text>,
        public -> Bool,
    }
}

table! {
    report (id) {
        id -> Integer,
        uuid -> Text,
        user_id -> Integer,
        project_id -> Integer,
        version_id -> Integer,
        testbed_id -> Integer,
        adapter_id -> Integer,
        start_time -> Timestamp,
        end_time -> Timestamp,
    }
}

table! {
    testbed (id) {
        id -> Integer,
        uuid -> Text,
        project_id -> Integer,
        name -> Text,
        slug -> Text,
        os_name -> Nullable<Text>,
        os_version -> Nullable<Text>,
        runtime_name -> Nullable<Text>,
        runtime_version -> Nullable<Text>,
        cpu -> Nullable<Text>,
        ram -> Nullable<Text>,
        disk -> Nullable<Text>,
    }
}

table! {
    user (id) {
        id -> Integer,
        uuid -> Text,
        name -> Text,
        slug -> Text,
        email -> Text,
    }
}

table! {
    version (id) {
        id -> Integer,
        uuid -> Text,
        branch_id -> Integer,
        count -> Integer,
        hash -> Nullable<Text>,
    }
}

joinable!(branch -> project (project_id));
joinable!(project -> user (owner_id));
joinable!(report -> adapter (adapter_id));
joinable!(report -> project (project_id));
joinable!(report -> testbed (testbed_id));
joinable!(report -> user (user_id));
joinable!(report -> version (version_id));
joinable!(testbed -> project (project_id));
joinable!(version -> branch (branch_id));

allow_tables_to_appear_in_same_query!(
    adapter,
    branch,
    project,
    report,
    testbed,
    user,
    version,
);
