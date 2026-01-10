use std::net::IpAddr;

use http::HeaderMap;
use slog::Logger;

pub(super) fn remote_ip(log: &Logger, headers: &HeaderMap) -> Option<IpAddr> {
    remote_ip_inner(headers)
        .inspect(|remote_ip| {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserIp);
            slog::info!(log, "Remote IP"; "remote_ip" => %remote_ip);
        })
        .or_else(|| {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserIpNotFound);
            None
        })
}

fn remote_ip_inner(headers: &HeaderMap) -> Option<IpAddr> {
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
