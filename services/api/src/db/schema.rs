table! {
    adapter (id) {
        id -> Integer,
        uuid -> Text,
        name -> Text,
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
        cpu -> Nullable<Text>,
        ram -> Nullable<Text>,
        disk -> Nullable<Text>,
    }
}

joinable!(report -> adapter (adapter_id));
joinable!(report -> testbed (testbed_id));

allow_tables_to_appear_in_same_query!(
    adapter,
    report,
    testbed,
);
