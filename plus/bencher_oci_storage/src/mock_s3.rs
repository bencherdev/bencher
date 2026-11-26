//! A stand-in S3 store for tests, which answers each request with a test's own answer.
//!
//! Like R2, it refuses a presigned request with 403 `SignatureDoesNotMatch` when
//! the signature does not cover what was sent, or when it leaves a copy header
//! unsigned. Its refusal echoes the access key ID and the host, as R2's does.

use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use md5::{Digest as _, Md5};
use object_store::signer::Url;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use sha2::Sha256;
use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;

pub(crate) const ACCESS_KEY_ID: &str = "AKIAMOCKKEYID0123456";
pub(crate) const SECRET_ACCESS_KEY: &str = "mock-secret-access-key";
pub(crate) const BUCKET: &str = "my-bucket";
pub(crate) const UPLOAD_ID: &str = "mock-upload-id";

/// The query encoding that `SigV4` canonicalizes with.
const QUERY_ENCODE_SET: AsciiSet = NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

#[derive(Debug, Clone)]
pub(crate) struct Request {
    pub method: String,
    /// The path and query, as sent.
    pub target: String,
    /// Lowercase names, in the order sent.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header, _value)| header == name)
            .map(|(_header, value)| value.as_str())
    }

    pub(crate) fn query(&self, name: &str) -> Option<String> {
        Url::parse(&format!("http://mock{}", self.target))
            .unwrap()
            .query_pairs()
            .find(|(key, _value)| key == name)
            .map(|(_key, value)| value.into_owned())
    }

    /// The path as sent, still percent-encoded.
    pub(crate) fn raw_path(&self) -> &str {
        self.target
            .split_once('?')
            .map_or(self.target.as_str(), |(path, _query)| path)
    }

    /// The object key the request names, as `object_store` writes it in a `Path`.
    pub(crate) fn key(&self) -> String {
        let path = percent_decode_str(self.raw_path())
            .decode_utf8()
            .unwrap()
            .into_owned();
        path.strip_prefix(&format!("/{BUCKET}/"))
            .unwrap_or_default()
            .to_owned()
    }

    pub(crate) fn part_number(&self) -> Option<u64> {
        self.query("partNumber")?.parse().ok()
    }

    pub(crate) fn is_list(&self) -> bool {
        self.method == "GET" && self.query("list-type").is_some()
    }

    pub(crate) fn is_part_copy(&self) -> bool {
        self.method == "PUT"
            && self.part_number().is_some()
            && self.header("x-amz-copy-source").is_some()
    }

    pub(crate) fn is_copy_object(&self) -> bool {
        self.method == "PUT"
            && self.part_number().is_none()
            && self.header("x-amz-copy-source").is_some()
    }

    pub(crate) fn is_abort(&self) -> bool {
        self.method == "DELETE" && self.query("uploadId").is_some()
    }

    pub(crate) fn is_create_multipart(&self) -> bool {
        self.method == "POST" && self.query("uploads").is_some()
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.method == "POST" && self.query("uploadId").is_some()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// The store hangs up once it has sent this answer.
    pub close: bool,
}

impl Response {
    pub(crate) fn new(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            close: false,
        }
    }

    /// A 200 that promises `length` bytes, sends `body`, and hangs up.
    pub(crate) fn cut_short(length: usize, body: &[u8]) -> Self {
        Self {
            body: body.to_vec(),
            close: true,
            ..Self::new(200)
        }
        .header("content-length", &length.to_string())
        .header("last-modified", "Fri, 20 Nov 2026 00:00:00 GMT")
        .header("etag", "\"0123456789abcdef0123456789abcdef\"")
    }

    pub(crate) fn xml(status: u16, body: &str) -> Self {
        Self {
            body: body.as_bytes().to_vec(),
            ..Self::new(status)
        }
        .header("content-type", "application/xml")
    }

    pub(crate) fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    /// No answer: the store hangs up once it has read the request.
    pub(crate) fn hang_up() -> Self {
        Self::new(0)
    }

    /// An S3 `<Error>` body with its code and message.
    pub(crate) fn error(status: u16, code: &str) -> Self {
        Self::xml(
            status,
            &format!("<Error><Code>{code}</Code><Message>{code} from the mock</Message></Error>"),
        )
    }
}

/// Each answer the store sent, in the order it sent them, with the request it answered.
#[derive(Debug, Default, Clone)]
pub(crate) struct Log(Arc<Mutex<Vec<(Request, u16)>>>);

impl Log {
    fn push(&self, request: Request, status: u16) {
        self.0.lock().unwrap().push((request, status));
    }

    /// Every answer but those to the startup sweep's lists.
    pub(crate) fn answers(&self) -> Vec<(Request, u16)> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|(request, _status)| !request.is_list())
            .cloned()
            .collect()
    }

    pub(crate) fn requests(&self) -> Vec<Request> {
        self.answers()
            .into_iter()
            .map(|(request, _status)| request)
            .collect()
    }
}

/// Serves `listener` until the test ends, answering each request with `answer`.
pub(crate) async fn serve<A, F>(listener: TcpListener, log: Log, answer: A)
where
    A: Fn(Request) -> F + Clone + Send + Sync + 'static,
    F: Future<Output = Response> + Send + 'static,
{
    let mut connections = JoinSet::new();
    while let Ok((socket, _peer)) = listener.accept().await {
        while connections.try_join_next().is_some() {}
        connections.spawn(serve_connection(socket, log.clone(), answer.clone()));
    }
}

async fn serve_connection<A, F>(socket: TcpStream, log: Log, answer: A)
where
    A: Fn(Request) -> F,
    F: Future<Output = Response>,
{
    let (reader, mut writer) = socket.into_split();
    let mut reader = BufReader::new(reader);
    while let Some(request) = read_request(&mut reader).await {
        let response = match refusal(&request) {
            Some(refusal) => refusal,
            None => answer(request.clone()).await,
        };
        log.push(request, response.status);
        if response.status == Response::hang_up().status
            || write_response(&mut writer, &response).await.is_err()
            || response.close
        {
            return;
        }
    }
}

async fn read_request<R>(reader: &mut BufReader<R>) -> Option<Request>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut line = String::new();
    if reader.read_line(&mut line).await.ok()? == 0 {
        return None;
    }
    let mut words = line.split_whitespace();
    let method = words.next()?.to_owned();
    let target = words.next()?.to_owned();

    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await.ok()?;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':')?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
    }

    let length = headers
        .iter()
        .find(|(name, _value)| name == "content-length")
        .and_then(|(_name, value)| value.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).await.ok()?;

    Some(Request {
        method,
        target,
        headers,
        body,
    })
}

async fn write_response<W>(writer: &mut W, response: &Response) -> std::io::Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut head = format!("HTTP/1.1 {} Mock\r\n", response.status);
    if !response
        .headers
        .iter()
        .any(|(name, _value)| name.eq_ignore_ascii_case("content-length"))
    {
        write!(head, "content-length: {}\r\n", response.body.len()).unwrap();
    }
    for (name, value) in &response.headers {
        write!(head, "{name}: {value}\r\n").unwrap();
    }
    head.push_str("\r\n");
    writer.write_all(head.as_bytes()).await?;
    writer.write_all(&response.body).await?;
    writer.flush().await
}

/// R2's refusal of a presigned request that its signature does not cover.
fn refusal(request: &Request) -> Option<Response> {
    request.query("X-Amz-Signature")?;
    let signed = request.query("X-Amz-SignedHeaders").unwrap_or_default();
    let signed: Vec<&str> = signed.split(';').collect();
    let copy_headers_signed = ["x-amz-copy-source", "x-amz-copy-source-range"]
        .iter()
        .all(|name| request.header(name).is_none() || signed.contains(name));
    if copy_headers_signed && signature(request) == request.query("X-Amz-Signature")? {
        return None;
    }
    Some(signature_does_not_match(request))
}

/// The message of R2's `SignatureDoesNotMatch` body.
pub(crate) const SIGNATURE_MESSAGE: &str = "The request signature we calculated does not match the signature you provided. Check your secret access key and signing method.";

/// A `SignatureDoesNotMatch` body like R2's, which echoes the access key ID and
/// the host, plain and hex-encoded.
pub(crate) fn signature_does_not_match(request: &Request) -> Response {
    let host = request.header("host").unwrap_or_default();
    let canonical_request = format!(
        "{}\n{}\n\nhost:{host}\n\nhost\nUNSIGNED-PAYLOAD",
        request.method,
        request.raw_path()
    );
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n20261120T000000Z\n20261120/auto/s3/aws4_request\n{}",
        hex::encode(Sha256::digest(canonical_request.as_bytes()))
    );
    Response::xml(
        403,
        &format!(
            "<Error><Code>SignatureDoesNotMatch</Code><Message>{SIGNATURE_MESSAGE}</Message><AWSAccessKeyId>{ACCESS_KEY_ID}</AWSAccessKeyId><StringToSign>{string_to_sign}</StringToSign><StringToSignBytes>{}</StringToSignBytes><CanonicalRequest>{canonical_request}</CanonicalRequest><CanonicalRequestBytes>{}</CanonicalRequestBytes></Error>",
            hex_bytes(&string_to_sign),
            hex_bytes(&canonical_request),
        ),
    )
}

/// A space-separated hex form, as S3 echoes the bytes it signed.
pub(crate) fn hex_bytes(text: &str) -> String {
    text.bytes()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The `SigV4` query signature of what the request sent.
fn signature(request: &Request) -> String {
    let mut query: Vec<(String, String)> = Url::parse(&format!("http://mock{}", request.target))
        .unwrap()
        .query_pairs()
        .filter(|(key, _value)| key != "X-Amz-Signature")
        .map(|(key, value)| {
            (
                utf8_percent_encode(&key, &QUERY_ENCODE_SET).to_string(),
                utf8_percent_encode(&value, &QUERY_ENCODE_SET).to_string(),
            )
        })
        .collect();
    query.sort();
    let query = query
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");

    let signed_headers = request.query("X-Amz-SignedHeaders").unwrap_or_default();
    let mut canonical_headers = String::new();
    for name in signed_headers.split(';') {
        writeln!(
            canonical_headers,
            "{name}:{}",
            request.header(name).unwrap_or_default()
        )
        .unwrap();
    }
    let canonical_request = format!(
        "{}\n{}\n{query}\n{canonical_headers}\n{signed_headers}\nUNSIGNED-PAYLOAD",
        request.method,
        request.raw_path(),
    );

    let date = request.query("X-Amz-Date").unwrap_or_default();
    let credential = request.query("X-Amz-Credential").unwrap_or_default();
    let scope = credential
        .split_once('/')
        .map(|(_key, scope)| scope.to_owned())
        .unwrap_or_default();
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{date}\n{scope}\n{}",
        hex::encode(Sha256::digest(canonical_request.as_bytes()))
    );

    let mut key = format!("AWS4{SECRET_ACCESS_KEY}").into_bytes();
    for part in scope.split('/') {
        key = hmac(&key, part.as_bytes()).to_vec();
    }
    hex::encode(hmac(&key, string_to_sign.as_bytes()))
}

fn hmac(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut block = [0; 64];
    block[..key.len()].copy_from_slice(key);
    let inner = Sha256::new()
        .chain_update(block.map(|byte| byte ^ 0x36))
        .chain_update(data)
        .finalize();
    Sha256::new()
        .chain_update(block.map(|byte| byte ^ 0x5c))
        .chain_update(inner)
        .finalize()
        .into()
}

/// The `ETag` the mock gives part `number` of a copy.
pub(crate) fn part_e_tag(number: u64) -> String {
    format!(
        "\"{}\"",
        hex::encode(Md5::digest(format!("part {number}").as_bytes()))
    )
}

/// The multipart `ETag` of the parts that a completion body lists, or `None`
/// when, as R2 would refuse, a part is out of order or not the part the mock copied.
pub(crate) fn completed_e_tag(body: &[u8]) -> Option<String> {
    let body = String::from_utf8_lossy(body).replace("&quot;", "\"");
    let element = |part: &str, name: &str| {
        part.split_once(&format!("<{name}>"))
            .and_then(|(_before, rest)| rest.split_once(&format!("</{name}>")))
            .map(|(value, _after)| value.to_owned())
    };
    let mut digests = Vec::new();
    for (number, part) in (1..).zip(body.split("<Part>").skip(1)) {
        let e_tag = element(part, "ETag")?;
        if element(part, "PartNumber")? != number.to_string() || e_tag != part_e_tag(number) {
            return None;
        }
        digests.push(hex::decode(e_tag.trim_matches('"')).ok()?);
    }
    Some(format!(
        "{}-{}",
        hex::encode(Md5::digest(digests.concat())),
        digests.len()
    ))
}

/// Answers as a bucket that holds `objects`, by key and size, would: copies
/// succeed, completions return the multipart `ETag` of the parts they list, and
/// nothing moves any bytes.
pub(crate) fn bucket(request: &Request, objects: &[(String, u64)]) -> Response {
    if request.is_list() {
        return Response::xml(
            200,
            "<ListBucketResult><IsTruncated>false</IsTruncated></ListBucketResult>",
        );
    }
    if request.is_create_multipart() {
        return Response::xml(
            200,
            &format!(
                "<InitiateMultipartUploadResult><Bucket>{BUCKET}</Bucket><Key>{}</Key><UploadId>{UPLOAD_ID}</UploadId></InitiateMultipartUploadResult>",
                request.key()
            ),
        );
    }
    if request.is_part_copy() {
        return Response::xml(
            200,
            &format!(
                "<CopyPartResult><LastModified>2026-11-20T00:00:00.000Z</LastModified><ETag>{}</ETag></CopyPartResult>",
                part_e_tag(request.part_number().unwrap_or_default())
            ),
        );
    }
    if request.is_complete() {
        return completed_e_tag(&request.body).map_or_else(
            || Response::error(400, "InvalidPart"),
            |e_tag| {
                Response::xml(
                    200,
                    &format!(
                        "<CompleteMultipartUploadResult><Bucket>{BUCKET}</Bucket><Key>{}</Key><ETag>\"{e_tag}\"</ETag></CompleteMultipartUploadResult>",
                        request.key()
                    ),
                )
            },
        );
    }
    if request.is_copy_object() {
        return Response::xml(
            200,
            "<CopyObjectResult><LastModified>2026-11-20T00:00:00.000Z</LastModified><ETag>\"0\"</ETag></CopyObjectResult>",
        );
    }
    if request.is_abort() {
        return Response::new(204);
    }
    if request.method == "HEAD" {
        let key = request.key();
        return match objects.iter().find(|(object, _size)| *object == key) {
            Some((_object, size)) => Response::new(200)
                .header("content-length", &size.to_string())
                .header("last-modified", "Fri, 20 Nov 2026 00:00:00 GMT")
                .header("etag", "\"0123456789abcdef0123456789abcdef\""),
            None => Response::new(404),
        };
    }
    Response::error(400, "UnexpectedRequest")
}
