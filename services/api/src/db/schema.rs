table! {
    adapter (id) {
        id -> Integer,
        uuid -> Text,
        name -> Text,
    }
}

table! {
    project (id) {
        id -> Integer,
        uuid -> Text,
        owner_id -> Integer,
        owner_default -> Bool,
        name -> Text,
        description -> Nullable<Text>,
        url -> Nullable<Text>,
    }
}

table! {
    report (id) {
        id -> Integer,
        uuid -> Text,
        project -> Nullable<Text>,
        testbed_id -> Nullable<Integer>,
        adapter_id -> Integer,
        start_time -> Timestamp,
        end_time -> Timestamp,
    }
}

table! {
    testbed (id) {
        id -> Integer,
        uuid -> Text,
        name -> Text,
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

joinable!(project -> user (owner_id));
joinable!(report -> adapter (adapter_id));
joinable!(report -> testbed (testbed_id));

allow_tables_to_appear_in_same_query!(
    adapter,
    project,
    report,
    testbed,
    user,
);
