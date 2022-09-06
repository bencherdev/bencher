table! {
    alert (id) {
        id -> Integer,
        uuid -> Text,
        perf_id -> Integer,
        threshold_id -> Integer,
        statistic_id -> Integer,
        side -> Bool,
        boundary -> Float,
        outlier -> Float,
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
    organization (id) {
        id -> Integer,
        uuid -> Text,
        name -> Text,
        slug -> Text,
    }
}

table! {
    organization_role (id) {
        id -> Integer,
        user_id -> Integer,
        organization_id -> Integer,
        role -> Text,
    }
}

table! {
    perf (id) {
        id -> Integer,
        uuid -> Text,
        report_id -> Integer,
        iteration -> Integer,
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
    project_role (id) {
        id -> Integer,
        user_id -> Integer,
        project_id -> Integer,
        role -> Text,
    }
}

table! {
    report (id) {
        id -> Integer,
        uuid -> Text,
        user_id -> Integer,
        version_id -> Integer,
        testbed_id -> Integer,
        adapter -> Integer,
        start_time -> BigInt,
        end_time -> BigInt,
    }
}

table! {
    resource (id) {
        id -> Integer,
        uuid -> Text,
        min -> Double,
        max -> Double,
        avg -> Double,
    }
}

table! {
    statistic (id) {
        id -> Integer,
        uuid -> Text,
        test -> Integer,
        sample_size -> Nullable<BigInt>,
        window -> Nullable<BigInt>,
        left_side -> Nullable<Float>,
        right_side -> Nullable<Float>,
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
    threshold (id) {
        id -> Integer,
        uuid -> Text,
        branch_id -> Integer,
        testbed_id -> Integer,
        kind -> Integer,
        statistic_id -> Integer,
    }
}

table! {
    throughput (id) {
        id -> Integer,
        uuid -> Text,
        lower_variance -> Double,
        upper_variance -> Double,
        events -> Double,
        unit_time -> BigInt,
    }
}

table! {
    token (id) {
        id -> Integer,
        uuid -> Text,
        user_id -> Integer,
        name -> Text,
        jwt -> Text,
        creation -> BigInt,
        expiration -> BigInt,
    }
}

table! {
    user (id) {
        id -> Integer,
        uuid -> Text,
        name -> Text,
        slug -> Text,
        email -> Text,
        admin -> Bool,
        locked -> Bool,
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

joinable!(alert -> perf (perf_id));
joinable!(alert -> statistic (statistic_id));
joinable!(alert -> threshold (threshold_id));
joinable!(benchmark -> project (project_id));
joinable!(branch -> project (project_id));
joinable!(organization_role -> organization (organization_id));
joinable!(organization_role -> user (user_id));
joinable!(perf -> benchmark (benchmark_id));
joinable!(perf -> latency (latency_id));
joinable!(perf -> report (report_id));
joinable!(perf -> throughput (throughput_id));
joinable!(project -> user (owner_id));
joinable!(project_role -> project (project_id));
joinable!(project_role -> user (user_id));
joinable!(report -> testbed (testbed_id));
joinable!(report -> user (user_id));
joinable!(report -> version (version_id));
joinable!(testbed -> project (project_id));
joinable!(threshold -> branch (branch_id));
joinable!(threshold -> statistic (statistic_id));
joinable!(threshold -> testbed (testbed_id));
joinable!(token -> user (user_id));
joinable!(version -> branch (branch_id));

allow_tables_to_appear_in_same_query!(
    alert,
    benchmark,
    branch,
    latency,
    organization,
    organization_role,
    perf,
    project,
    project_role,
    report,
    resource,
    statistic,
    testbed,
    threshold,
    throughput,
    token,
    user,
    version,
);
