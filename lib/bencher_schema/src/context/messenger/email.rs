use std::{fmt, sync::Arc, time::Duration};

use bencher_json::Secret;
use bencher_json::system::config::JsonSmtp;
use mail_send::{SmtpClientBuilder, mail_builder::MessageBuilder};
use slog::Logger;
use tokio::sync::Mutex;

use super::Message;
use super::body::FmtBody as _;

pub const DEFAULT_SMTP_PORT: u16 = 587;
/// How long a send waits on the SMTP server, its connection included, before it gives up.
const SEND_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct Email {
    client: Arc<Mutex<Client>>,
    from_name: Option<String>,
    #[expect(clippy::struct_field_names, reason = "from_email is more descriptive")]
    from_email: String,
}

impl From<JsonSmtp> for Email {
    fn from(smtp: JsonSmtp) -> Self {
        let JsonSmtp {
            hostname,
            port,
            insecure_host,
            starttls,
            username,
            secret,
            from_name,
            from_email,
        } = smtp;
        let client_builder = ClientBuilder {
            hostname: hostname.into(),
            port: port.unwrap_or(DEFAULT_SMTP_PORT),
            insecure_host: insecure_host.unwrap_or_default(),
            starttls: starttls.unwrap_or(true),
            username: username.into(),
            secret,
        };
        Self {
            client: Arc::new(Mutex::new(client_builder.into())),
            from_name: Some(from_name.into()),
            from_email: from_email.into(),
        }
    }
}

impl Email {
    pub fn send(&self, log: &Logger, message: Message) {
        let from_email = self.from_email.clone();
        let to_email = message.to_email.clone();
        let message_builder = self.message_builder(log, message);

        slog::info!(log, "Spawning email task: from {from_email} to {to_email}");
        let send_log = log.clone();
        let send_client = self.client.clone();
        tokio::spawn(async move {
            let mut client = send_client.lock().await;
            match client.send(&send_log, message_builder).await {
                Ok(()) => {
                    slog::info!(send_log, "Email sent: from {from_email} to {to_email}");
                    #[cfg(feature = "otel")]
                    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::EmailSend);
                },
                Err(err) => {
                    slog::error!(
                        send_log,
                        "Failed to send email from {from_email} to {to_email}: {err}"
                    );
                    #[cfg(feature = "otel")]
                    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::EmailSendFailed);
                    #[cfg(feature = "sentry")]
                    sentry::capture_error(&err);
                },
            }
        });
    }

    /// Send the message and wait until it is sent, for at most [`SEND_TIMEOUT`].
    /// Its one log line comes before the send and names only its addresses, so nothing logged
    /// depends on how the send went; the caller answers for a failure.
    pub async fn send_and_wait(
        &self,
        log: &Logger,
        message: Message,
    ) -> Result<(), mail_send::Error> {
        slog::info!(
            log,
            "Sending email: from {from_email} to {to_email}",
            from_email = &self.from_email,
            to_email = &message.to_email
        );
        let message_builder = self.message_builder(log, message);
        let sent = tokio::time::timeout(SEND_TIMEOUT, async {
            self.client.lock().await.send(log, message_builder).await
        })
        .await
        .unwrap_or_else(|_elapsed| Err(mail_send::Error::Timeout));
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(if sent.is_ok() {
            bencher_otel::ApiCounter::EmailSend
        } else {
            bencher_otel::ApiCounter::EmailSendFailed
        });
        sent
    }

    fn message_builder(&self, log: &Logger, message: Message) -> MessageBuilder<'static> {
        let mut message_builder = MessageBuilder::new();

        message_builder = if let Some(name) = self.from_name.clone() {
            message_builder.from((name, self.from_email.clone()))
        } else {
            message_builder.from(self.from_email.clone())
        };

        message_builder = if let Some(name) = message.to_name {
            message_builder.to((name, message.to_email))
        } else {
            message_builder.to(message.to_email)
        };

        if let Some(subject) = message.subject {
            message_builder = message_builder.subject(subject);
        }

        if let Some(body) = message.body {
            message_builder = message_builder
                .text_body(body.text())
                .html_body(body.html(log));
        }

        message_builder
    }
}

#[derive(Debug, Clone)]
struct ClientBuilder {
    hostname: String,
    port: u16,
    insecure_host: bool,
    starttls: bool,
    username: String,
    secret: Secret,
}

impl ClientBuilder {
    fn build(&self) -> Result<SmtpClientBuilder<String>, mail_send::Error> {
        let mut builder = SmtpClientBuilder::new(self.hostname.clone(), self.port)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        if self.insecure_host {
            builder = builder.allow_invalid_certs();
        }

        Ok(builder
            .implicit_tls(!self.starttls)
            .timeout(SEND_TIMEOUT)
            .credentials((self.username.clone(), String::from(self.secret.clone()))))
    }
}

struct Client {
    builder: ClientBuilder,
    handle: Option<mail_send::SmtpClient<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("client_builder", &self.builder)
            .field(
                "handle",
                &if self.handle.is_some() {
                    "Connected"
                } else {
                    "Disconnected"
                },
            )
            .finish()
    }
}

impl From<ClientBuilder> for Client {
    fn from(builder: ClientBuilder) -> Self {
        Self {
            builder,
            handle: None,
        }
    }
}

impl Client {
    /// The connection is kept only once a send on it succeeds. A send that fails or is cancelled
    /// can leave a transaction open or a reply unread, which would break every send after it, so
    /// the next send connects again.
    async fn send(
        &mut self,
        log: &Logger,
        message_builder: MessageBuilder<'_>,
    ) -> Result<(), mail_send::Error> {
        let mut client = if let Some(mut client) = self.handle.take() {
            if client.noop().await.is_ok() {
                client
            } else {
                slog::debug!(log, "Refreshing client");
                self.builder.build()?.connect().await?
            }
        } else {
            slog::debug!(log, "Creating new client");
            self.builder.build()?.connect().await?
        };

        client.send(message_builder).await?;
        self.handle = Some(client);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bencher_json::system::config::JsonSmtp;
    use futures_concurrency::future::Race as _;
    use tokio::{
        io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader},
        net::{TcpListener, TcpStream},
        sync::mpsc,
        time::Instant,
    };
    use tokio_rustls::{
        TlsAcceptor,
        rustls::{
            ServerConfig,
            pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject as _},
        },
        server::TlsStream,
    };

    use super::{ClientBuilder, Email, SEND_TIMEOUT};
    use crate::context::{Body, Button, ButtonBody, Message, Messenger};

    /// A self-signed certificate for 127.0.0.1, for these tests alone, made with
    /// `openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -days 36500
    /// -subj /CN=localhost -addext subjectAltName=IP:127.0.0.1 -keyout smtp_key.pem
    /// -out smtp_cert.pem`.
    const SMTP_CERT: &str = include_str!("test/smtp_cert.pem");
    const SMTP_KEY: &str = include_str!("test/smtp_key.pem");

    /// The test server refuses this recipient.
    const REFUSED: &str = "ada@refused.example";
    /// The test server never answers for this recipient.
    const STALLED: &str = "ada@stalled.example";
    const DELIVERED: &str = "grace@example.com";

    // A refused recipient leaves its connection inside an open transaction, so the next email
    // gets through only on a new connection.
    #[tokio::test]
    async fn refused_recipient_leaves_the_next_email_a_new_connection() {
        let (server, mut seen) = SmtpServer::bind().await;
        let email = server.email();
        let log = discard();
        let sends = async {
            let refused = email.send_and_wait(&log, message(REFUSED)).await;
            assert!(
                matches!(refused, Err(mail_send::Error::UnexpectedReply(_))),
                "{refused:?}"
            );
            email.send_and_wait(&log, message(DELIVERED)).await.unwrap();
        };
        (server.serve(), sends).race().await;
        drop(server);

        assert_eq!(seen.recv().await, Some(Seen::Refused(REFUSED.to_owned())));
        assert_eq!(
            seen.recv().await,
            Some(Seen::Delivered(DELIVERED.to_owned()))
        );
        assert_eq!(seen.recv().await, None);
    }

    // A send cancelled while it waits on a reply, as its timeout cancels it, leaves that reply
    // unread on its connection, so the next email gets through only on a new connection.
    #[tokio::test]
    async fn cancelled_send_leaves_the_next_email_a_new_connection() {
        let (server, mut seen) = SmtpServer::bind().await;
        let email = server.email();
        let log = discard();
        let sends = async {
            let stalled = (
                async { Err::<Option<Seen>, _>(email.send_and_wait(&log, message(STALLED)).await) },
                async { Ok(seen.recv().await) },
            )
                .race()
                .await;
            assert!(matches!(stalled, Ok(Some(Seen::Stalled(_)))), "{stalled:?}");
            email.send_and_wait(&log, message(DELIVERED)).await.unwrap();
        };
        (server.serve(), sends).race().await;
        drop(server);

        assert_eq!(
            seen.recv().await,
            Some(Seen::Delivered(DELIVERED.to_owned()))
        );
        assert_eq!(seen.recv().await, None);
    }

    // A detached send shares the client, so its refused recipient leaves the next email a new
    // connection too.
    #[tokio::test]
    async fn refused_detached_email_leaves_the_next_email_a_new_connection() {
        let (server, mut seen) = SmtpServer::bind().await;
        let email = server.email();
        let log = discard();
        let sends = async {
            email.send(&log, message(REFUSED));
            assert_eq!(seen.recv().await, Some(Seen::Refused(REFUSED.to_owned())));
            // The detached send holds the client until the refusal reaches it, so this send
            // starts only after it.
            email.send_and_wait(&log, message(DELIVERED)).await.unwrap();
        };
        (server.serve(), sends).race().await;
        drop(server);

        assert_eq!(
            seen.recv().await,
            Some(Seen::Delivered(DELIVERED.to_owned()))
        );
        assert_eq!(seen.recv().await, None);
    }

    // An email's subject and body can carry a link's secret, so a send, whether it waits or not,
    // logs neither, at any level.
    #[tokio::test]
    async fn sends_log_neither_subject_nor_body() {
        const SECRET: &str = "a-link-secret-0123456789";
        let (server, mut seen) = SmtpServer::bind().await;
        let messenger = Messenger::Email(server.email());
        let lines = Lines::default();
        let log = slog::Logger::root(lines.clone(), slog::o!());
        let with_secret = || Message {
            to_name: None,
            to_email: DELIVERED.to_owned(),
            subject: Some(format!("Subject {SECRET}")),
            body: Some(Body::Button(Box::new(ButtonBody {
                title: SECRET.to_owned(),
                preheader: String::new(),
                greeting: String::new(),
                pre_body: SECRET.to_owned(),
                button: Some(Button {
                    text: "Join".to_owned(),
                    url: format!("https://bencher.example/auth/login#invite={SECRET}"),
                }),
                clipboard: None,
                post_body: String::new(),
                closing: String::new(),
                signature: String::new(),
                settings_url: String::new(),
            }))),
        };
        let sends = async {
            messenger.send(&log, with_secret());
            assert_eq!(
                seen.recv().await,
                Some(Seen::Delivered(DELIVERED.to_owned()))
            );
            messenger.send_and_wait(&log, with_secret()).await.unwrap();
        };
        (server.serve(), sends).race().await;
        drop(server);

        assert_eq!(
            seen.recv().await,
            Some(Seen::Delivered(DELIVERED.to_owned()))
        );
        let logged = lines.0.lock().unwrap().clone();
        assert!(
            logged.iter().any(|line| line.contains(DELIVERED)),
            "{logged:?}"
        );
        assert!(
            logged.iter().all(|line| !line.contains(SECRET)),
            "{logged:?}"
        );
    }

    /// Every line logged, its message and its key-value pairs.
    #[derive(Clone, Default)]
    struct Lines(Arc<std::sync::Mutex<Vec<String>>>);

    impl slog::Drain for Lines {
        type Ok = ();
        type Err = slog::Never;

        fn log(
            &self,
            record: &slog::Record<'_>,
            values: &slog::OwnedKVList,
        ) -> Result<(), slog::Never> {
            use slog::KV as _;

            let mut line = Line(record.msg().to_string());
            let _serialized = record
                .kv()
                .serialize(record, &mut line)
                .and_then(|()| values.serialize(record, &mut line));
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(line.0);
            Ok(())
        }
    }

    struct Line(String);

    impl slog::Serializer for Line {
        fn emit_arguments(
            &mut self,
            key: slog::Key,
            val: &std::fmt::Arguments<'_>,
        ) -> slog::Result {
            use std::fmt::Write as _;

            write!(self.0, " {key}={val}").map_err(slog::Error::Fmt)
        }
    }

    // A server that accepts the connection and never answers holds a send for no longer than the
    // timeout.
    #[tokio::test(start_paused = true)]
    async fn send_and_wait_gives_up_on_a_silent_server() {
        let (port, silent) = silent_server().await;
        let email = Email::from(JsonSmtp {
            hostname: "127.0.0.1".parse().unwrap(),
            port: Some(port),
            insecure_host: None,
            starttls: None,
            username: "bencher".parse().unwrap(),
            secret: "an-smtp-secret".parse().unwrap(),
            from_name: "Bencher".parse().unwrap(),
            from_email: "bencher@example.com".parse().unwrap(),
        });
        let message = Message {
            to_name: None,
            to_email: "ada@example.com".to_owned(),
            subject: Some("Subject".to_owned()),
            body: None,
        };

        let log = slog::Logger::root(slog::Discard, slog::o!());
        let started = Instant::now();
        let (sent, _accepted) = tokio::join!(email.send_and_wait(&log, message), silent.accept());
        assert!(matches!(sent, Err(mail_send::Error::Timeout)), "{sent:?}");
        assert!(started.elapsed() <= SEND_TIMEOUT, "{:?}", started.elapsed());
    }

    // The connection gives up on a silent server by itself, which bounds the sends nobody waits
    // on.
    #[tokio::test(start_paused = true)]
    async fn connection_gives_up_on_a_silent_server() {
        let (port, silent) = silent_server().await;
        let builder = ClientBuilder {
            hostname: "127.0.0.1".to_owned(),
            port,
            insecure_host: false,
            starttls: true,
            username: "bencher".to_owned(),
            secret: "an-smtp-secret".parse().unwrap(),
        };

        let started = Instant::now();
        let (connected, _accepted) = tokio::join!(
            async { builder.build().unwrap().connect().await.map(drop) },
            silent.accept()
        );
        assert!(
            matches!(connected, Err(mail_send::Error::Timeout)),
            "{connected:?}"
        );
        assert!(started.elapsed() <= SEND_TIMEOUT, "{:?}", started.elapsed());
    }

    /// A listener the test accepts a connection on and never answers.
    async fn silent_server() -> (u16, TcpListener) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        (port, listener)
    }

    /// What the test server did with a recipient.
    #[derive(Debug, PartialEq, Eq)]
    enum Seen {
        Refused(String),
        Stalled(String),
        Delivered(String),
    }

    /// An SMTP server over implicit TLS that serves one connection at a time. As a mail relay
    /// does, it keeps a transaction open after it refuses the recipient, so a later MAIL on the
    /// same connection is refused as nested.
    struct SmtpServer {
        listener: TcpListener,
        acceptor: TlsAcceptor,
        seen: mpsc::Sender<Seen>,
    }

    impl SmtpServer {
        async fn bind() -> (Self, mpsc::Receiver<Seen>) {
            let config = ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(
                    vec![CertificateDer::from_pem_slice(SMTP_CERT.as_bytes()).unwrap()],
                    PrivateKeyDer::from_pem_slice(SMTP_KEY.as_bytes()).unwrap(),
                )
                .unwrap();
            let (seen, receiver) = mpsc::channel(8);
            let server = Self {
                listener: TcpListener::bind("127.0.0.1:0").await.unwrap(),
                acceptor: TlsAcceptor::from(Arc::new(config)),
                seen,
            };
            (server, receiver)
        }

        /// A client of this server, which takes its self-signed certificate.
        fn email(&self) -> Email {
            Email::from(JsonSmtp {
                hostname: "127.0.0.1".parse().unwrap(),
                port: Some(self.listener.local_addr().unwrap().port()),
                insecure_host: Some(true),
                starttls: Some(false),
                username: "bencher".parse().unwrap(),
                secret: "an-smtp-secret".parse().unwrap(),
                from_name: "Bencher".parse().unwrap(),
                from_email: "bencher@example.com".parse().unwrap(),
            })
        }

        /// Serve one connection after another, until dropped or its listener fails.
        async fn serve(&self) {
            while let Ok((tcp, _addr)) = self.listener.accept().await {
                if let Ok(tls) = self.acceptor.accept(tcp).await {
                    self.session(BufReader::new(tls)).await;
                }
            }
        }

        /// Answer one connection until the client drops it.
        async fn session(&self, mut stream: BufReader<TlsStream<TcpStream>>) {
            let mut transaction: Option<Vec<String>> = None;
            let mut reply = Some("220 localhost ESMTP");
            loop {
                if let Some(reply) = reply
                    && !Self::reply(&mut stream, reply).await
                {
                    return;
                }
                let Some(line) = Self::line(&mut stream).await else {
                    return;
                };
                reply = if line.starts_with("EHLO") {
                    Some("250-localhost\r\n250 AUTH PLAIN")
                } else if line.starts_with("AUTH PLAIN") {
                    Some("235 2.7.0 Authentication successful")
                } else if line == "NOOP" {
                    Some("250 2.0.0 Ok")
                } else if line.starts_with("MAIL FROM:") {
                    if transaction.is_some() {
                        Some("503 5.5.1 Error: nested MAIL command")
                    } else {
                        transaction = Some(Vec::new());
                        Some("250 2.1.0 Ok")
                    }
                } else if let Some(to) = line
                    .strip_prefix("RCPT TO:<")
                    .and_then(|to| to.strip_suffix('>'))
                {
                    if to == REFUSED {
                        self.seen.send(Seen::Refused(to.to_owned())).await.unwrap();
                        Some("550 5.1.1 Recipient address rejected")
                    } else if to == STALLED {
                        self.seen.send(Seen::Stalled(to.to_owned())).await.unwrap();
                        None
                    } else if let Some(recipients) = &mut transaction {
                        recipients.push(to.to_owned());
                        Some("250 2.1.5 Ok")
                    } else {
                        Some("503 5.5.1 Error: need MAIL command")
                    }
                } else if line == "DATA" {
                    let Some(recipients) = transaction.take().filter(|r| !r.is_empty()) else {
                        reply = Some("554 5.5.1 Error: no valid recipients");
                        continue;
                    };
                    if !Self::reply(&mut stream, "354 End data with <CR><LF>.<CR><LF>").await {
                        return;
                    }
                    while Self::line(&mut stream)
                        .await
                        .is_some_and(|line| line != ".")
                    {}
                    for to in recipients {
                        self.seen.send(Seen::Delivered(to)).await.unwrap();
                    }
                    Some("250 2.0.0 Ok: queued")
                } else {
                    Some("502 5.5.2 Error: command not recognized")
                };
            }
        }

        async fn reply(stream: &mut BufReader<TlsStream<TcpStream>>, reply: &str) -> bool {
            let tls = stream.get_mut();
            tls.write_all(format!("{reply}\r\n").as_bytes())
                .await
                .is_ok()
                && tls.flush().await.is_ok()
        }

        /// The next line the client sent, or `None` once it is gone.
        async fn line(stream: &mut BufReader<TlsStream<TcpStream>>) -> Option<String> {
            let mut line = String::new();
            match stream.read_line(&mut line).await {
                Ok(0) | Err(_) => None,
                Ok(_) => Some(line.trim_end().to_owned()),
            }
        }
    }

    fn message(to_email: &str) -> Message {
        Message {
            to_name: None,
            to_email: to_email.to_owned(),
            subject: Some("Subject".to_owned()),
            body: None,
        }
    }

    fn discard() -> slog::Logger {
        slog::Logger::root(slog::Discard, slog::o!())
    }
}
