macro_rules! http_error {
    ($message:expr, $($field:tt)*) => {
        // |e| {
        {
            let m = format!($message, $($field)*);
            tracing::error!("{m}");
            // tracing::error!("{m}: {e}");
            dropshot::HttpError::for_bad_request(
                Some(http::status::StatusCode::BAD_REQUEST.to_string()),
                m,
            )
        }
        // }
    };
    ($message:expr) => {$crate::util::error::http_error!($message,)};
}

pub(crate) use http_error;
