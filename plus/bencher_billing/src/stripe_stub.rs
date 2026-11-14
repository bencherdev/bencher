//! A Stripe API on localhost for offline tests of the biller's calls.
//!
//! The stub holds Stripe objects fully expanded and answers a read with the fields the
//! request did not expand collapsed to their ids, as Stripe does. It records every
//! request, and a subscription update merges its `metadata` into the stored object.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    task::JoinSet,
};

/// One request the stub answered.
#[derive(Debug, Clone)]
pub struct StubRequest {
    pub method: String,
    pub path: String,
    /// The decoded query, as `key=value` pairs.
    pub query: Vec<(String, String)>,
    /// The decoded form body, as `key=value` pairs.
    pub form: Vec<(String, String)>,
}

impl StubRequest {
    fn expand(&self) -> Vec<String> {
        self.query
            .iter()
            .filter(|(key, _)| key.starts_with("expand"))
            .map(|(_, value)| value.clone())
            .collect()
    }
}

/// The Stripe objects the stub serves, by id.
#[derive(Debug, Default)]
pub struct StubObjects {
    pub subscriptions: HashMap<String, Value>,
    pub sessions: HashMap<String, Value>,
}

#[derive(Debug, Default)]
struct StubState {
    objects: StubObjects,
    requests: Vec<StubRequest>,
}

pub struct StripeStub {
    url: String,
    state: Arc<Mutex<StubState>>,
    _server: JoinSet<()>,
}

impl StripeStub {
    pub async fn new(objects: StubObjects) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(StubState {
            objects,
            requests: Vec::new(),
        }));
        let mut server = JoinSet::new();
        let shared = Arc::clone(&state);
        server.spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let Some(request) = read_request(&mut stream).await else {
                    continue;
                };
                let (status, body) = {
                    let mut state = shared.lock().unwrap();
                    let answer = answer(&mut state.objects, &request);
                    state.requests.push(request);
                    answer
                };
                let response = format!(
                    "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {length}\r\nconnection: close\r\n\r\n{body}",
                    reason = if status == 200 { "OK" } else { "Not Found" },
                    length = body.len(),
                );
                stream
                    .write_all(response.as_bytes())
                    .await
                    .unwrap_or_default();
                stream.shutdown().await.unwrap_or_default();
            }
        });
        Self {
            url,
            state,
            _server: server,
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn requests(&self) -> Vec<StubRequest> {
        self.state.lock().unwrap().requests.clone()
    }

    /// The subscription as the stub now holds it.
    pub fn subscription(&self, id: &str) -> Value {
        self.state.lock().unwrap().objects.subscriptions[id].clone()
    }
}

fn answer(objects: &mut StubObjects, request: &StubRequest) -> (u16, String) {
    let expand = request.expand();
    let segments: Vec<&str> = request.path.trim_matches('/').split('/').collect();
    match (request.method.as_str(), segments.as_slice()) {
        ("GET", ["v1", "subscriptions", id]) => objects.subscriptions.get(*id).map_or_else(
            || missing("subscription", id),
            |subscription| found(&collapse_subscription(subscription.clone(), &expand, "")),
        ),
        ("POST", ["v1", "subscriptions", id]) => {
            let Some(subscription) = objects.subscriptions.get_mut(*id) else {
                return missing("subscription", id);
            };
            for (key, value) in &request.form {
                if let Some(key) = key
                    .strip_prefix("metadata[")
                    .and_then(|key| key.strip_suffix(']'))
                {
                    subscription["metadata"][key] = json!(value);
                }
            }
            found(&collapse_subscription(subscription.clone(), &expand, ""))
        },
        ("GET", ["v1", "checkout", "sessions", id]) => {
            let Some(session) = objects.sessions.get(*id) else {
                return missing("checkout session", id);
            };
            let mut session = session.clone();
            let subscription_id = session["subscription"].as_str().unwrap_or_default();
            if expand.iter().any(|path| path == "subscription")
                && let Some(subscription) = objects.subscriptions.get(subscription_id)
            {
                session["subscription"] =
                    collapse_subscription(subscription.clone(), &expand, "subscription.");
            }
            found(&session)
        },
        _ => missing("resource", &request.path),
    }
}

/// A subscription with every field that `expand` (under `prefix`) does not name collapsed
/// to its id, and the item prices without their tiers unless the tiers are expanded.
fn collapse_subscription(mut subscription: Value, expand: &[String], prefix: &str) -> Value {
    let expanded = |path: &str| expand.iter().any(|e| e == &format!("{prefix}{path}"));
    if !expanded("customer.invoice_settings.default_payment_method") {
        collapse(&mut subscription["customer"]["invoice_settings"]["default_payment_method"]);
    }
    if !(expanded("customer") || expanded("customer.invoice_settings.default_payment_method")) {
        collapse(&mut subscription["customer"]);
    }
    if !expanded("default_payment_method") {
        collapse(&mut subscription["default_payment_method"]);
    }
    if !expanded("items.data.price.tiers")
        && let Some(items) = subscription["items"]["data"].as_array_mut()
    {
        for item in items {
            if let Some(price) = item["price"].as_object_mut() {
                price.remove("tiers");
            }
        }
    }
    subscription
}

fn collapse(field: &mut Value) {
    if let Some(id) = field.get("id").cloned() {
        *field = id;
    }
}

fn found(value: &Value) -> (u16, String) {
    (200, value.to_string())
}

fn missing(kind: &str, id: &str) -> (u16, String) {
    (
        404,
        json!({
            "error": {
                "type": "invalid_request_error",
                "code": "resource_missing",
                "message": format!("No such {kind}: '{id}'"),
            }
        })
        .to_string(),
    )
}

async fn read_request(stream: &mut TcpStream) -> Option<StubRequest> {
    let mut buffer = Vec::new();
    let mut chunk = [0; 4096];
    let head_end = loop {
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let head = String::from_utf8_lossy(&buffer[..head_end]).into_owned();
    let length = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or_default();
    while buffer.len() < head_end + length {
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    let body = String::from_utf8_lossy(&buffer[head_end..head_end + length]).into_owned();
    let mut start = head.lines().next()?.split(' ');
    let method = start.next()?.to_owned();
    let target = start.next()?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    Some(StubRequest {
        method,
        path: path.to_owned(),
        query: pairs(query),
        form: pairs(&body),
    })
}

fn pairs(encoded: &str) -> Vec<(String, String)> {
    encoded
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(key), decode(value))
        })
        .collect()
}

fn decode(encoded: &str) -> String {
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                if let Some(byte) = bytes
                    .get(index + 1..index + 3)
                    .and_then(|hex| std::str::from_utf8(hex).ok())
                    .and_then(|hex| u8::from_str_radix(hex, 16).ok())
                {
                    decoded.push(byte);
                    index += 3;
                    continue;
                }
                decoded.push(b'%');
            },
            b'+' => decoded.push(b' '),
            byte => decoded.push(byte),
        }
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

pub const PRO_PRODUCT: &str = "prod_pro";
pub const ENTERPRISE_PRODUCT: &str = "prod_enterprise";
pub const BARE_METAL_PRODUCT: &str = "prod_bare_metal";
/// A Stripe timestamp inside the current period of every fixture.
const NOW: i64 = 1_790_000_000;

/// A per unit price of `unit_amount_decimal` cents on `product`, with the whole cent
/// `unit_amount` that Stripe sets beside it when there is one.
pub fn price(id: &str, product: &str, unit_amount_decimal: &str) -> Value {
    json!({
        "id": id,
        "object": "price",
        "active": true,
        "billing_scheme": "per_unit",
        "created": NOW,
        "currency": "usd",
        "livemode": false,
        "metadata": {},
        "product": product,
        "type": "recurring",
        "unit_amount": unit_amount_decimal.parse::<i64>().ok(),
        "unit_amount_decimal": unit_amount_decimal,
    })
}

/// A tiered price on `product`, in `tiers_mode`.
pub fn tiered_price(id: &str, product: &str, tiers_mode: &str, tiers: &Value) -> Value {
    json!({
        "id": id,
        "object": "price",
        "active": true,
        "billing_scheme": "tiered",
        "created": NOW,
        "currency": "usd",
        "livemode": false,
        "metadata": {},
        "product": product,
        "type": "recurring",
        "tiers_mode": tiers_mode,
        "tiers": tiers,
    })
}

pub fn item(price: &Value) -> Value {
    let id = format!("si_{}", price["id"].as_str().unwrap_or_default());
    json!({
        "id": id,
        "object": "subscription_item",
        "created": NOW,
        "current_period_start": NOW - 86_400,
        "current_period_end": NOW + 86_400,
        "discounts": [],
        "metadata": {},
        "plan": {
            "id": price["id"],
            "object": "plan",
            "active": true,
            "billing_scheme": price["billing_scheme"],
            "created": NOW,
            "currency": "usd",
            "interval": "month",
            "interval_count": 1,
            "livemode": false,
            "usage_type": "metered",
        },
        "price": price,
        "subscription": "sub_stub",
    })
}

/// A customer with `metadata`, whose invoices default to `payment_method` when given.
pub fn customer(metadata: &Value, payment_method: Option<&Value>) -> Value {
    json!({
        "id": "cus_stub",
        "object": "customer",
        "created": NOW,
        "livemode": false,
        "metadata": metadata,
        "name": "Muriel Bagge",
        "email": "muriel@nowhere.example",
        "invoice_settings": { "default_payment_method": payment_method },
    })
}

pub fn card(id: &str) -> Value {
    json!({
        "id": id,
        "object": "payment_method",
        "billing_details": {},
        "created": NOW,
        "livemode": false,
        "type": "card",
        "card": {
            "brand": "visa",
            "exp_month": 8,
            "exp_year": 2028,
            "funding": "credit",
            "last4": "4242",
        },
    })
}

pub fn bank_account(id: &str) -> Value {
    json!({
        "id": id,
        "object": "payment_method",
        "billing_details": {},
        "created": NOW,
        "livemode": false,
        "type": "us_bank_account",
    })
}

/// A subscription with `items`, `metadata`, and `customer`, with `payment_method` as its
/// own default payment method when given.
pub fn subscription(
    id: &str,
    status: &str,
    collection_method: &str,
    items: Vec<Value>,
    metadata: Value,
    customer: Value,
    payment_method: Option<Value>,
) -> Value {
    let mut subscription = json!({
        "id": id,
        "object": "subscription",
        "automatic_tax": { "enabled": false },
        "billing_cycle_anchor": NOW - 86_400,
        "billing_mode": { "type": "classic" },
        "billing_schedules": [],
        "cancel_at_period_end": false,
        "collection_method": collection_method,
        "created": NOW - 86_400,
        "currency": "usd",
        "discounts": [],
        "invoice_settings": { "issuer": { "type": "self" } },
        "items": {
            "object": "list",
            "has_more": false,
            "url": format!("/v1/subscription_items?subscription={id}"),
        },
        "livemode": false,
        "start_date": NOW - 86_400,
        "status": status,
    });
    subscription["customer"] = customer;
    subscription["default_payment_method"] = payment_method.unwrap_or_default();
    subscription["items"]["data"] = Value::Array(items);
    subscription["metadata"] = metadata;
    subscription
}

pub fn session(id: &str, subscription: &str) -> Value {
    json!({
        "id": id,
        "object": "checkout.session",
        "automatic_tax": { "enabled": false },
        "created": NOW,
        "custom_fields": [],
        "custom_text": {},
        "expires_at": NOW + 86_400,
        "livemode": false,
        "mode": "subscription",
        "payment_method_types": ["card"],
        "payment_status": "paid",
        "shipping_options": [],
        "subscription": subscription,
    })
}
