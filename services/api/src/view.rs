use crate::schema::{
    alert, benchmark, boundary, branch, measure, metric, model, project, project_role, reference,
    reference_version, report, report_benchmark, testbed, threshold, version,
};

diesel::table! {
    metric_boundary (metric_id) {
        metric_id -> Integer,
        metric_uuid -> Text,
        report_benchmark_id -> Integer,
        measure_id -> Integer,
        value -> Double,
        lower_value -> Nullable<Double>,
        upper_value -> Nullable<Double>,
        boundary_id -> Nullable<Integer>,
        boundary_uuid -> Nullable<Text>,
        threshold_id -> Nullable<Integer>,
        model_id -> Nullable<Integer>,
        baseline -> Nullable<Double>,
        lower_limit -> Nullable<Double>,
        upper_limit -> Nullable<Double>,
    }
}

diesel::joinable!(metric_boundary -> measure (measure_id));
diesel::joinable!(metric_boundary -> report_benchmark (report_benchmark_id));
diesel::joinable!(metric_boundary -> model (model_id));
diesel::joinable!(metric_boundary -> threshold (threshold_id));

diesel::allow_tables_to_appear_in_same_query!(metric_boundary, alert);
diesel::allow_tables_to_appear_in_same_query!(metric_boundary, benchmark);
diesel::allow_tables_to_appear_in_same_query!(metric_boundary, boundary);
diesel::allow_tables_to_appear_in_same_query!(metric_boundary, branch);
diesel::allow_tables_to_appear_in_same_query!(metric_boundary, measure);
diesel::allow_tables_to_appear_in_same_query!(metric_boundary, metric);
diesel::allow_tables_to_appear_in_same_query!(metric_boundary, model);
diesel::allow_tables_to_appear_in_same_query!(metric_boundary, project);
diesel::allow_tables_to_appear_in_same_query!(metric_boundary, project_role);
diesel::allow_tables_to_appear_in_same_query!(metric_boundary, reference);
diesel::allow_tables_to_appear_in_same_query!(metric_boundary, reference_version);
diesel::allow_tables_to_appear_in_same_query!(metric_boundary, report);
diesel::allow_tables_to_appear_in_same_query!(metric_boundary, report_benchmark);
diesel::allow_tables_to_appear_in_same_query!(metric_boundary, testbed);
diesel::allow_tables_to_appear_in_same_query!(metric_boundary, threshold);
diesel::allow_tables_to_appear_in_same_query!(metric_boundary, version);
