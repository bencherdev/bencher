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
    boundary (id) {
        id -> Integer,
        uuid -> Text,
        perf_id -> Integer,
        threshold_id -> Integer,
        statistic_id -> Integer,
        boundary_side -> Bool,
        boundary_limit -> Double,
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
    branch_version (id) {
        id -> Integer,
        branch_id -> Integer,
        version_id -> Integer,
    }
}

diesel::table! {
    metric (id) {
        id -> Integer,
        uuid -> Text,
        perf_id -> Integer,
        metric_kind_id -> Integer,
        value -> Double,
        lower_bound -> Nullable<Double>,
        upper_bound -> Nullable<Double>,
    }
}

diesel::table! {
    metric_kind (id) {
        id -> Integer,
        uuid -> Text,
        project_id -> Integer,
        name -> Text,
        slug -> Text,
        units -> Text,
    }
}

diesel::table! {
    organization (id) {
        id -> Integer,
        uuid -> Text,
        name -> Text,
        slug -> Text,
        subscription -> Nullable<Text>,
        license -> Nullable<Text>,
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
    }
}

diesel::table! {
    project (id) {
        id -> Integer,
        uuid -> Text,
        organization_id -> Integer,
        name -> Text,
        slug -> Text,
        url -> Nullable<Text>,
        visibility -> Integer,
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
        branch_id -> Integer,
        version_id -> Integer,
        testbed_id -> Integer,
        adapter -> Integer,
        start_time -> BigInt,
        end_time -> BigInt,
    }
}

diesel::table! {
    statistic (id) {
        id -> Integer,
        uuid -> Text,
        test -> Integer,
        min_sample_size -> Nullable<BigInt>,
        max_sample_size -> Nullable<BigInt>,
        window -> Nullable<BigInt>,
        left_side -> Nullable<Double>,
        right_side -> Nullable<Double>,
    }
}

diesel::table! {
    testbed (id) {
        id -> Integer,
        uuid -> Text,
        project_id -> Integer,
        name -> Text,
        slug -> Text,
    }
}

diesel::table! {
    threshold (id) {
        id -> Integer,
        uuid -> Text,
        branch_id -> Integer,
        testbed_id -> Integer,
        metric_kind_id -> Integer,
        statistic_id -> Integer,
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
        number -> Integer,
        hash -> Nullable<Text>,
    }
}

diesel::joinable!(alert -> perf (perf_id));
diesel::joinable!(alert -> statistic (statistic_id));
diesel::joinable!(alert -> threshold (threshold_id));
diesel::joinable!(benchmark -> project (project_id));
diesel::joinable!(boundary -> perf (perf_id));
diesel::joinable!(boundary -> statistic (statistic_id));
diesel::joinable!(boundary -> threshold (threshold_id));
diesel::joinable!(branch -> project (project_id));
diesel::joinable!(branch_version -> branch (branch_id));
diesel::joinable!(branch_version -> version (version_id));
diesel::joinable!(metric -> metric_kind (metric_kind_id));
diesel::joinable!(metric -> perf (perf_id));
diesel::joinable!(metric_kind -> project (project_id));
diesel::joinable!(organization_role -> organization (organization_id));
diesel::joinable!(organization_role -> user (user_id));
diesel::joinable!(perf -> benchmark (benchmark_id));
diesel::joinable!(perf -> report (report_id));
diesel::joinable!(project -> organization (organization_id));
diesel::joinable!(project_role -> project (project_id));
diesel::joinable!(project_role -> user (user_id));
diesel::joinable!(report -> testbed (testbed_id));
diesel::joinable!(report -> user (user_id));
diesel::joinable!(report -> version (version_id));
diesel::joinable!(testbed -> project (project_id));
diesel::joinable!(threshold -> branch (branch_id));
diesel::joinable!(threshold -> metric_kind (metric_kind_id));
diesel::joinable!(threshold -> statistic (statistic_id));
diesel::joinable!(threshold -> testbed (testbed_id));
diesel::joinable!(token -> user (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    alert,
    benchmark,
    boundary,
    branch,
    branch_version,
    metric,
    metric_kind,
    organization,
    organization_role,
    perf,
    project,
    project_role,
    report,
    statistic,
    testbed,
    threshold,
    token,
    user,
    version,
);
