macro_rules! http_error {
    ($message:expr, $($field:tt)*) => {
        {
            dropshot::HttpError::for_bad_request(
                Some(http::status::StatusCode::BAD_REQUEST.to_string()),
                format!($message, $($field)*),
            )
        }
    };
    ($message:expr) => {$crate::util::error::http_error!($message,)};
}

pub(crate) use http_error;

macro_rules! map_http_error {
    ($message:expr, $($field:tt)*) => {
        |e| {
            let m = format!($message, $($field)*);
            tracing::info!("{m}: {e}");
            dropshot::HttpError::for_bad_request(
                Some(http::status::StatusCode::BAD_REQUEST.to_string()),
                m,
            )
        }
    };
    ($message:expr) => {$crate::util::error::map_http_error!($message,)};
}

pub(crate) use map_http_error;
