use std::net::IpAddr;

use http::HeaderMap;
use slog::Logger;

pub(super) fn remote_ip(log: &Logger, headers: &HeaderMap, relayed: bool) -> Option<IpAddr> {
    remote_ip_inner(headers, relayed)
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

fn remote_ip_inner(headers: &HeaderMap, relayed: bool) -> Option<IpAddr> {
    if relayed && let ip @ Some(_) = relayed_ip(headers) {
        return ip;
    }

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

/// The reader's address as the relay forwards it, only when the request names exactly one.
fn relayed_ip(headers: &HeaderMap) -> Option<IpAddr> {
    let mut values = headers.get_all(bencher_json::RELAY_CLIENT_IP).iter();
    let (Some(value), None) = (values.next(), values.next()) else {
        return None;
    };
    value.to_str().ok().and_then(|s| s.trim().parse().ok())
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use http::{HeaderMap, HeaderValue};

    use super::remote_ip_inner;

    // A relayed request that names no reader, or more than one, is read as any other.
    #[test]
    fn relayed_ip_is_read_only_when_relayed() {
        let reader: IpAddr = "203.0.113.7".parse().unwrap();
        let edge: IpAddr = "198.51.100.1".parse().unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            bencher_json::RELAY_CLIENT_IP,
            HeaderValue::from_static("203.0.113.7"),
        );
        headers.insert("Fly-Client-IP", HeaderValue::from_static("198.51.100.1"));

        assert_eq!(remote_ip_inner(&headers, true), Some(reader));
        assert_eq!(remote_ip_inner(&headers, false), Some(edge));

        headers.append(
            bencher_json::RELAY_CLIENT_IP,
            HeaderValue::from_static("203.0.113.8"),
        );
        assert_eq!(remote_ip_inner(&headers, true), Some(edge));

        headers.remove(bencher_json::RELAY_CLIENT_IP);
        assert_eq!(remote_ip_inner(&headers, true), Some(edge));
    }
}
