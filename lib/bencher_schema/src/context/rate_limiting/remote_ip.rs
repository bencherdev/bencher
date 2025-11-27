pub(super) fn remote_ip(headers: &http::HeaderMap) -> Option<std::net::IpAddr> {
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
