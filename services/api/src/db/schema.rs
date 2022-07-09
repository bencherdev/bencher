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
        testbed -> Nullable<Text>,
        adapter_id -> Integer,
        start_time -> Timestamp,
        end_time -> Timestamp,
    }
}

joinable!(report -> adapter (adapter_id));

allow_tables_to_appear_in_same_query!(
    adapter,
    report,
);
