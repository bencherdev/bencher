pub const MAX_PERMUTATIONS: usize = 3;

macro_rules! metrics_query {
    ($($number:ident),*) => {
        pub enum MetricsQuery<$($number),*, N3> {
            N0,
            $($number($number)),*,
            N3(N3)
        }
    }
}

macro_rules! generate_increment_metrics_query {
    ($(($number:ident, $next:ident)),*) => {
        #[macro_export]
        macro_rules! increment_metrics_query {
            ($metrics_query:ident, $select_query:ident) => {
                match $metrics_query {
                    MetricsQuery::N0 => {
                        $metrics_query = MetricsQuery::N1($select_query
                        // The ORDER BY clause is applied to the combined result set, not within the individual result set.
                        // So we need to order by branch, testbed, and benchmark first to keep the results grouped.
                        // Order by the version number so that the oldest version is first.
                        // Because multiple reports can use the same version (via git hash), order by the start time next.
                        // Then within a report order by the iteration number.
                        .order((
                            schema::branch::name,
                            schema::testbed::name,
                            schema::benchmark::name,
                            schema::version::number,
                            schema::report::start_time,
                            schema::perf::iteration,
                        )));
                    },
                    $(MetricsQuery::$number(query) => {
                        $metrics_query = MetricsQuery::$next(query.union_all($select_query));
                    }),*
                    MetricsQuery::N3(_) => {
                        debug_assert!(false, "Ended up at the maximum metrics query count 3 for max {MAX_PERMUTATIONS} permutations");
                    },
                }
            }
        }
    }
}

macro_rules! generate_match_metrics_query {
    ($($number:ident),*) => {
        #[macro_export]
        macro_rules! match_metrics_query {
            ($conn:ident, $project:ident, $metric_kind_id:ident, $metrics_query:ident) => {
                match $metrics_query {
                    MetricsQuery::N0 => (Vec::new(), None),
                    $(MetricsQuery::$number(query) => query
                        .load::<PerfQuery>($conn)
                        .map_err(resource_not_found_err!(Metric, ($project, $metric_kind_id)))?
                        .into_iter()
                        .fold((Vec::new(), None), |(results, perf_metrics), query| {
                            into_perf_metrics($project, results, perf_metrics, query)
                        })),*,
                    MetricsQuery::N3(query) => query
                        .load::<PerfQuery>($conn)
                        .map_err(resource_not_found_err!(Metric, ($project, $metric_kind_id)))?
                        .into_iter()
                        .fold((Vec::new(), None), |(results, perf_metrics), query| {
                            into_perf_metrics($project, results, perf_metrics, query)
                        })
                }
            }
        }
    }
}

// metrics_query! {
//     1, 2, 3
// }

// generate_increment_metrics_query! {
//     (1, 2),
//     (2, 3)
// }

// generate_match_metrics_query! {
//     1, 2, 3
// }

macro_rules! meta_generate {
    ($(($number:literal, $next:literal)),*) => {
        paste::paste! {
            metrics_query! { $([<N $number>]),* }
            generate_increment_metrics_query! { $(([<N $number>], [<N $next>])),* }
            pub(crate) use increment_metrics_query;
            generate_match_metrics_query! { $([<N $number>]),* }
            pub(crate) use match_metrics_query;
        }
    }
}

meta_generate! {
    (1, 2),
    (2, 3)
}
