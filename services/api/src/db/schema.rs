table! {
    report (id) {
        id -> Integer,
        project -> Nullable<Text>,
        testbed -> Nullable<Text>,
        start_time -> Nullable<Timestamp>,
        end_time -> Timestamp,
    }
}
