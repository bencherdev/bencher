table! {
    report (id) {
        id -> Int4,
        date_time -> Timestamptz,
        metrics -> Jsonb,
        hash -> Int8,
        length -> Int4,
    }
}
