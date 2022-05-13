table! {
    report (id) {
        id -> Int4,
        date_time -> Timestamptz,
        reports -> Jsonb,
        hash -> Int4,
        length -> Int4,
    }
}
