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
        archived -> Nullable<BigInt>,
    }
}

diesel::table! {
    boundary (id) {
        id -> Integer,
        uuid -> Text,
        metric_id -> Integer,
        threshold_id -> Integer,
        model_id -> Integer,
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
        head_id -> Nullable<Integer>,
        created -> BigInt,
        modified -> BigInt,
        archived -> Nullable<BigInt>,
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
        archived -> Nullable<BigInt>,
    }
}

diesel::table! {
    metric (id) {
        id -> Integer,
        uuid -> Text,
        report_benchmark_id -> Integer,
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
    plot (id) {
        id -> Integer,
        uuid -> Text,
        project_id -> Integer,
        rank -> BigInt,
        title -> Nullable<Text>,
        lower_value -> Bool,
        upper_value -> Bool,
        lower_boundary -> Bool,
        upper_boundary -> Bool,
        x_axis -> Integer,
        window -> BigInt,
        created -> BigInt,
        modified -> BigInt,
    }
}

diesel::table! {
    plot_benchmark (plot_id, benchmark_id) {
        plot_id -> Integer,
        benchmark_id -> Integer,
        rank -> BigInt,
    }
}

diesel::table! {
    plot_branch (plot_id, branch_id) {
        plot_id -> Integer,
        branch_id -> Integer,
        rank -> BigInt,
    }
}

diesel::table! {
    plot_measure (plot_id, measure_id) {
        plot_id -> Integer,
        measure_id -> Integer,
        rank -> BigInt,
    }
}

diesel::table! {
    plot_testbed (plot_id, testbed_id) {
        plot_id -> Integer,
        testbed_id -> Integer,
        rank -> BigInt,
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
    reference (id) {
        id -> Integer,
        uuid -> Text,
        branch_id -> Integer,
        start_point_id -> Nullable<Integer>,
        created -> BigInt,
        replaced -> Nullable<BigInt>,
    }
}

diesel::table! {
    reference_version (id) {
        id -> Integer,
        reference_id -> Integer,
        version_id -> Integer,
    }
}

diesel::table! {
    report (id) {
        id -> Integer,
        uuid -> Text,
        user_id -> Integer,
        project_id -> Integer,
        reference_id -> Integer,
        version_id -> Integer,
        testbed_id -> Integer,
        adapter -> Integer,
        start_time -> BigInt,
        end_time -> BigInt,
        created -> BigInt,
    }
}

diesel::table! {
    report_benchmark (id) {
        id -> Integer,
        uuid -> Text,
        report_id -> Integer,
        iteration -> Integer,
        benchmark_id -> Integer,
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
        archived -> Nullable<BigInt>,
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
diesel::joinable!(measure -> project (project_id));
diesel::joinable!(metric -> measure (measure_id));
diesel::joinable!(metric -> report_benchmark (report_benchmark_id));
diesel::joinable!(organization_role -> organization (organization_id));
diesel::joinable!(organization_role -> user (user_id));
diesel::joinable!(plot -> project (project_id));
diesel::joinable!(plot_benchmark -> benchmark (benchmark_id));
diesel::joinable!(plot_benchmark -> plot (plot_id));
diesel::joinable!(plot_branch -> branch (branch_id));
diesel::joinable!(plot_branch -> plot (plot_id));
diesel::joinable!(plot_measure -> measure (measure_id));
diesel::joinable!(plot_measure -> plot (plot_id));
diesel::joinable!(plot_testbed -> plot (plot_id));
diesel::joinable!(plot_testbed -> testbed (testbed_id));
diesel::joinable!(project -> organization (organization_id));
diesel::joinable!(project_role -> project (project_id));
diesel::joinable!(project_role -> user (user_id));
diesel::joinable!(reference_version -> reference (reference_id));
diesel::joinable!(reference_version -> version (version_id));
diesel::joinable!(report -> project (project_id));
diesel::joinable!(report -> reference (reference_id));
diesel::joinable!(report -> testbed (testbed_id));
diesel::joinable!(report -> user (user_id));
diesel::joinable!(report -> version (version_id));
diesel::joinable!(report_benchmark -> benchmark (benchmark_id));
diesel::joinable!(report_benchmark -> report (report_id));
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
    measure,
    metric,
    model,
    organization,
    organization_role,
    plan,
    plot,
    plot_benchmark,
    plot_branch,
    plot_measure,
    plot_testbed,
    project,
    project_role,
    reference,
    reference_version,
    report,
    report_benchmark,
    server,
    testbed,
    threshold,
    token,
    user,
    version,
);
