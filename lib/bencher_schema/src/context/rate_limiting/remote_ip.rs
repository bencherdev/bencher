pub(super) fn remote_ip(headers: &http::HeaderMap) -> Option<std::net::IpAddr> {
    if let Some(remote_ip) = remote_ip_inner(headers) {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserIp);

        Some(remote_ip)
    } else {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserIpNotFound);

        None
    }
}

fn remote_ip_inner(headers: &http::HeaderMap) -> Option<std::net::IpAddr> {
    // https://fly.io/docs/networking/request-headers/#fly-client-ip
    if let ip @ Some(_) = headers
        .get("Fly-Client-IP")
        .or_else(|| headers.get("fly-client-ip"))
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse().ok())
    {
        return ip;
    }

    // https://fly.io/docs/networking/request-headers/#x-forwarded-for
    if let ip @ Some(_) = headers
        .get("X-Forwarded-For")
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').map(str::trim).find(|s| !s.is_empty()))
        .and_then(|s| s.parse().ok())
    {
        return ip;
    }

    None
}
