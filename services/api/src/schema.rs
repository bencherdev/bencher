// @generated automatically by Diesel CLI.

diesel::table! {
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

diesel::table! {
    benchmark (id) {
        id -> Integer,
        uuid -> Text,
        project_id -> Integer,
        name -> Text,
    }
}

diesel::table! {
    branch (id) {
        id -> Integer,
        uuid -> Text,
        project_id -> Integer,
        name -> Text,
        slug -> Text,
    }
}

diesel::table! {
    latency (id) {
        id -> Integer,
        uuid -> Text,
        lower_variance -> BigInt,
        upper_variance -> BigInt,
        duration -> BigInt,
    }
}

diesel::table! {
    organization (id) {
        id -> Integer,
        uuid -> Text,
        name -> Text,
        slug -> Text,
    }
}

diesel::table! {
    organization_role (id) {
        id -> Integer,
        user_id -> Integer,
        organization_id -> Integer,
        role -> Text,
    }
}

diesel::table! {
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

diesel::table! {
    project (id) {
        id -> Integer,
        uuid -> Text,
        organization_id -> Integer,
        name -> Text,
        slug -> Text,
        description -> Nullable<Text>,
        url -> Nullable<Text>,
        public -> Bool,
    }
}

diesel::table! {
    project_role (id) {
        id -> Integer,
        user_id -> Integer,
        project_id -> Integer,
        role -> Text,
    }
}

diesel::table! {
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

diesel::table! {
    resource (id) {
        id -> Integer,
        uuid -> Text,
        min -> Double,
        max -> Double,
        avg -> Double,
    }
}

diesel::table! {
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

diesel::table! {
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

diesel::table! {
    threshold (id) {
        id -> Integer,
        uuid -> Text,
        branch_id -> Integer,
        testbed_id -> Integer,
        kind -> Integer,
        statistic_id -> Integer,
    }
}

diesel::table! {
    throughput (id) {
        id -> Integer,
        uuid -> Text,
        lower_variance -> Double,
        upper_variance -> Double,
        events -> Double,
        unit_time -> BigInt,
    }
}

diesel::table! {
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

diesel::table! {
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

diesel::table! {
    version (id) {
        id -> Integer,
        uuid -> Text,
        branch_id -> Integer,
        number -> Integer,
        hash -> Nullable<Text>,
    }
}

diesel::joinable!(alert -> perf (perf_id));
diesel::joinable!(alert -> statistic (statistic_id));
diesel::joinable!(alert -> threshold (threshold_id));
diesel::joinable!(benchmark -> project (project_id));
diesel::joinable!(branch -> project (project_id));
diesel::joinable!(organization_role -> organization (organization_id));
diesel::joinable!(organization_role -> user (user_id));
diesel::joinable!(perf -> benchmark (benchmark_id));
diesel::joinable!(perf -> latency (latency_id));
diesel::joinable!(perf -> report (report_id));
diesel::joinable!(perf -> throughput (throughput_id));
diesel::joinable!(project -> organization (organization_id));
diesel::joinable!(project_role -> project (project_id));
diesel::joinable!(project_role -> user (user_id));
diesel::joinable!(report -> testbed (testbed_id));
diesel::joinable!(report -> user (user_id));
diesel::joinable!(report -> version (version_id));
diesel::joinable!(testbed -> project (project_id));
diesel::joinable!(threshold -> branch (branch_id));
diesel::joinable!(threshold -> statistic (statistic_id));
diesel::joinable!(threshold -> testbed (testbed_id));
diesel::joinable!(token -> user (user_id));
diesel::joinable!(version -> branch (branch_id));

diesel::allow_tables_to_appear_in_same_query!(
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
