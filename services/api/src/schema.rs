// @generated automatically by Diesel CLI.

diesel::table! {
    alert (id) {
        id -> Integer,
        uuid -> Text,
        boundary_id -> Integer,
        boundary_limit -> Bool,
        status -> Integer,
        modified -> BigInt,
    }
}

diesel::table! {
    benchmark (id) {
        id -> Integer,
        uuid -> Text,
        project_id -> Integer,
        name -> Text,
        slug -> Text,
        created -> BigInt,
        modified -> BigInt,
    }
}

diesel::table! {
    boundary (id) {
        id -> Integer,
        uuid -> Text,
        threshold_id -> Integer,
        model_id -> Integer,
        metric_id -> Integer,
        baseline -> Nullable<Double>,
        lower_limit -> Nullable<Double>,
        upper_limit -> Nullable<Double>,
    }
}

diesel::table! {
    branch (id) {
        id -> Integer,
        uuid -> Text,
        project_id -> Integer,
        name -> Text,
        slug -> Text,
        created -> BigInt,
        modified -> BigInt,
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
    measure (id) {
        id -> Integer,
        uuid -> Text,
        project_id -> Integer,
        name -> Text,
        slug -> Text,
        units -> Text,
        created -> BigInt,
        modified -> BigInt,
    }
}

diesel::table! {
    metric (id) {
        id -> Integer,
        uuid -> Text,
        perf_id -> Integer,
        measure_id -> Integer,
        value -> Double,
        lower_value -> Nullable<Double>,
        upper_value -> Nullable<Double>,
    }
}

diesel::table! {
    model (id) {
        id -> Integer,
        uuid -> Text,
        threshold_id -> Integer,
        test -> Integer,
        min_sample_size -> Nullable<BigInt>,
        max_sample_size -> Nullable<BigInt>,
        window -> Nullable<BigInt>,
        lower_boundary -> Nullable<Double>,
        upper_boundary -> Nullable<Double>,
        created -> BigInt,
        replaced -> Nullable<BigInt>,
    }
}

diesel::table! {
    organization (id) {
        id -> Integer,
        uuid -> Text,
        name -> Text,
        slug -> Text,
        license -> Nullable<Text>,
        created -> BigInt,
        modified -> BigInt,
    }
}

diesel::table! {
    organization_role (id) {
        id -> Integer,
        user_id -> Integer,
        organization_id -> Integer,
        role -> Text,
        created -> BigInt,
        modified -> BigInt,
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
    plan (id) {
        id -> Integer,
        organization_id -> Integer,
        metered_plan -> Nullable<Text>,
        licensed_plan -> Nullable<Text>,
        license -> Nullable<Text>,
        created -> BigInt,
        modified -> BigInt,
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
        created -> BigInt,
        modified -> BigInt,
    }
}

diesel::table! {
    project_role (id) {
        id -> Integer,
        user_id -> Integer,
        project_id -> Integer,
        role -> Text,
        created -> BigInt,
        modified -> BigInt,
    }
}

diesel::table! {
    report (id) {
        id -> Integer,
        uuid -> Text,
        user_id -> Integer,
        project_id -> Integer,
        branch_id -> Integer,
        version_id -> Integer,
        testbed_id -> Integer,
        adapter -> Integer,
        start_time -> BigInt,
        end_time -> BigInt,
        created -> BigInt,
    }
}

diesel::table! {
    server (id) {
        id -> Integer,
        uuid -> Text,
        created -> BigInt,
    }
}

diesel::table! {
    testbed (id) {
        id -> Integer,
        uuid -> Text,
        project_id -> Integer,
        name -> Text,
        slug -> Text,
        created -> BigInt,
        modified -> BigInt,
    }
}

diesel::table! {
    threshold (id) {
        id -> Integer,
        uuid -> Text,
        project_id -> Integer,
        branch_id -> Integer,
        testbed_id -> Integer,
        measure_id -> Integer,
        model_id -> Nullable<Integer>,
        created -> BigInt,
        modified -> BigInt,
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
        created -> BigInt,
        modified -> BigInt,
    }
}

diesel::table! {
    version (id) {
        id -> Integer,
        uuid -> Text,
        project_id -> Integer,
        number -> Integer,
        hash -> Nullable<Text>,
    }
}

diesel::joinable!(alert -> boundary (boundary_id));
diesel::joinable!(benchmark -> project (project_id));
diesel::joinable!(boundary -> metric (metric_id));
diesel::joinable!(boundary -> model (model_id));
diesel::joinable!(boundary -> threshold (threshold_id));
diesel::joinable!(branch -> project (project_id));
diesel::joinable!(branch_version -> branch (branch_id));
diesel::joinable!(branch_version -> version (version_id));
diesel::joinable!(measure -> project (project_id));
diesel::joinable!(metric -> measure (measure_id));
diesel::joinable!(metric -> perf (perf_id));
diesel::joinable!(organization_role -> organization (organization_id));
diesel::joinable!(organization_role -> user (user_id));
diesel::joinable!(perf -> benchmark (benchmark_id));
diesel::joinable!(perf -> report (report_id));
diesel::joinable!(project -> organization (organization_id));
diesel::joinable!(project_role -> project (project_id));
diesel::joinable!(project_role -> user (user_id));
diesel::joinable!(report -> branch (branch_id));
diesel::joinable!(report -> project (project_id));
diesel::joinable!(report -> testbed (testbed_id));
diesel::joinable!(report -> user (user_id));
diesel::joinable!(report -> version (version_id));
diesel::joinable!(testbed -> project (project_id));
diesel::joinable!(threshold -> branch (branch_id));
diesel::joinable!(threshold -> measure (measure_id));
diesel::joinable!(threshold -> project (project_id));
diesel::joinable!(threshold -> testbed (testbed_id));
diesel::joinable!(token -> user (user_id));
diesel::joinable!(version -> project (project_id));

diesel::allow_tables_to_appear_in_same_query!(
    alert,
    benchmark,
    boundary,
    branch,
    branch_version,
    measure,
    metric,
    model,
    organization,
    organization_role,
    perf,
    plan,
    project,
    project_role,
    report,
    server,
    testbed,
    threshold,
    token,
    user,
    version,
);
