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
    latency (id) {
        id -> Integer,
        uuid -> Text,
        lower_variance -> BigInt,
        upper_variance -> BigInt,
        duration -> BigInt,
    }
}

table! {
    min_max_avg (id) {
        id -> Integer,
        uuid -> Text,
        min -> Double,
        max -> Double,
        avg -> Double,
    }
}

table! {
    perf (id) {
        id -> Integer,
        uuid -> Text,
        report_id -> Integer,
        benchmark_id -> Integer,
        latency_id -> Nullable<Integer>,
        throughput_id -> Nullable<Integer>,
        compute_id -> Nullable<Integer>,
        memory_id -> Nullable<Integer>,
        storage_id -> Nullable<Integer>,
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
        start_time -> BigInt,
        end_time -> BigInt,
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
    throughput (id) {
        id -> Integer,
        uuid -> Text,
        lower_events -> Double,
        upper_events -> Double,
        unit_time -> BigInt,
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

table! {
    z_score (id) {
        id -> Integer,
        uuid -> Text,
        branch_id -> Integer,
        population -> Nullable<Integer>,
        deviation -> Float,
        active -> Bool,
    }
}

table! {
    z_score_alert (id) {
        id -> Integer,
        uuid -> Text,
        z_score_id -> Integer,
        perf_id -> Integer,
    }
}

joinable!(benchmark -> project (project_id));
joinable!(branch -> project (project_id));
joinable!(perf -> benchmark (benchmark_id));
joinable!(perf -> latency (latency_id));
joinable!(perf -> report (report_id));
joinable!(perf -> throughput (throughput_id));
joinable!(project -> user (owner_id));
joinable!(report -> adapter (adapter_id));
joinable!(report -> testbed (testbed_id));
joinable!(report -> user (user_id));
joinable!(report -> version (version_id));
joinable!(testbed -> project (project_id));
joinable!(version -> branch (branch_id));
joinable!(z_score -> branch (branch_id));
joinable!(z_score_alert -> perf (perf_id));
joinable!(z_score_alert -> z_score (z_score_id));

allow_tables_to_appear_in_same_query!(
    adapter,
    benchmark,
    branch,
    latency,
    min_max_avg,
    perf,
    project,
    report,
    testbed,
    throughput,
    user,
    version,
    z_score,
    z_score_alert,
);
