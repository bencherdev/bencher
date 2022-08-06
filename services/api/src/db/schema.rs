table! {
    adapter (id) {
        id -> Integer,
        uuid -> Text,
        name -> Text,
    }
}

table! {
    benchmark (id) {
        id -> Integer,
        uuid -> Text,
        project_id -> Integer,
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
    branch_benchmark (id) {
        id -> Integer,
        branch_id -> Integer,
        benchmark_id -> Integer,
    }
}

table! {
    perf (id) {
        id -> Integer,
        uuid -> Text,
        report_id -> Integer,
        branch_benchmark_id -> Integer,
        duration -> Nullable<Integer>,
        lower_variance -> Nullable<Integer>,
        upper_variance -> Nullable<Integer>,
        lower_events -> Nullable<Float>,
        upper_events -> Nullable<Float>,
        unit_time -> Nullable<Integer>,
        min_cpu -> Nullable<Float>,
        max_cpu -> Nullable<Float>,
        avg_cpu -> Nullable<Float>,
        min_memory -> Nullable<Float>,
        max_memory -> Nullable<Float>,
        avg_memory -> Nullable<Float>,
        min_disk -> Nullable<Float>,
        max_disk -> Nullable<Float>,
        avg_disk -> Nullable<Float>,
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
        number -> Integer,
        hash -> Nullable<Text>,
    }
}

joinable!(benchmark -> project (project_id));
joinable!(branch -> project (project_id));
joinable!(branch_benchmark -> benchmark (benchmark_id));
joinable!(branch_benchmark -> branch (branch_id));
joinable!(perf -> branch_benchmark (branch_benchmark_id));
joinable!(perf -> report (report_id));
joinable!(project -> user (owner_id));
joinable!(report -> adapter (adapter_id));
joinable!(report -> testbed (testbed_id));
joinable!(report -> user (user_id));
joinable!(report -> version (version_id));
joinable!(testbed -> project (project_id));
joinable!(version -> branch (branch_id));

allow_tables_to_appear_in_same_query!(
    adapter,
    benchmark,
    branch,
    branch_benchmark,
    perf,
    project,
    report,
    testbed,
    user,
    version,
);
