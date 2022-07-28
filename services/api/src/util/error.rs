macro_rules! http_error {
    ($message:expr) => {
        dropshot::HttpError::for_bad_request(
            Some(http::status::StatusCode::BAD_REQUEST.to_string()),
            $message.into(),
        )
    };
}

pub(crate) use http_error;
