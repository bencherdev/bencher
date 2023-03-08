use std::sync::{Arc, Mutex, RwLock, Weak};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
};

use log::{debug, error, info, trace, warn};

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value as Json};

use element::Element;
use point::Point;
use tokio::time::{sleep, Duration};

use crate::protocol::cdp::{
    types::{Event, Method},
    Browser, Debugger, Emulation, Fetch, Input, Log, Network, Page, Profiler, Runtime, Target, DOM,
};
use crate::wait::Wait;

use Runtime::AddBinding;

use base64::Engine;

use Input::DispatchKeyEvent;

use Page::{AddScriptToEvaluateOnNewDocument, Navigate, SetInterceptFileChooserDialog};

use Target::AttachToTarget;

use DOM::{Node, NodeId};

use Target::{TargetID, TargetInfo};

use Log::ViolationSetting;

use Fetch::{
    events::RequestPausedEvent, AuthChallengeResponse, ContinueRequest, ContinueWithAuth,
    FailRequest, FulfillRequest,
};

use Network::{
    events::LoadingFailedEventParams, events::ResponseReceivedEventParams, Cookie, GetResponseBody,
    GetResponseBodyReturnObject, SetExtraHTTPHeaders, SetUserAgentOverride,
};

use crate::{wait, ChromeError};

use crate::types::{Bounds, CurrentBounds, PrintToPdfOptions, RemoteError};

use super::transport::SessionId;
use crate::browser::transport::Transport;

pub mod element;
mod keys;
pub mod point;

#[derive(Debug)]
pub enum RequestPausedDecision {
    Fulfill(FulfillRequest),
    Fail(FailRequest),
    Continue(Option<ContinueRequest>),
}

#[rustfmt::skip]
pub type ResponseHandler = Box<
    dyn Fn(
        ResponseReceivedEventParams,
        &dyn Fn() -> Result<
            GetResponseBodyReturnObject,
            Error,
        >,
    ) + Send
    + Sync,
>;

#[rustfmt::skip]
pub type LoadingFailedHandler = Box<
    dyn Fn(
        ResponseReceivedEventParams,
        LoadingFailedEventParams
    ) + Send
    + Sync,
>;

type SyncSendEvent = dyn EventListener<Event> + Send + Sync;
pub trait RequestInterceptor {
    fn intercept(
        &self,
        transport: Arc<Transport>,
        session_id: SessionId,
        event: RequestPausedEvent,
    ) -> RequestPausedDecision;
}

impl<F> RequestInterceptor for F
where
    F: Fn(Arc<Transport>, SessionId, RequestPausedEvent) -> RequestPausedDecision + Send + Sync,
{
    fn intercept(
        &self,
        transport: Arc<Transport>,
        session_id: SessionId,
        event: RequestPausedEvent,
    ) -> RequestPausedDecision {
        self(transport, session_id, event)
    }
}

type RequestIntercept = dyn RequestInterceptor + Send + Sync;

pub trait EventListener<T> {
    fn on_event(&self, event: &T);
}

impl<T, F: Fn(&T) + Send + Sync> EventListener<T> for F {
    fn on_event(&self, event: &T) {
        self(event);
    }
}

pub trait Binding {
    fn call_binding(&self, data: Json);
}

impl<T: Fn(Json) + Send + Sync> Binding for T {
    fn call_binding(&self, data: Json) {
        self(data);
    }
}

pub type SafeBinding = dyn Binding + Send + Sync;

pub type FunctionBinding = HashMap<String, Arc<SafeBinding>>;

// type SyncSendEvent = dyn EventListener<Event> + Send + Sync;

/// A handle to a single page. Exposes methods for simulating user actions (clicking,
/// typing), and also for getting information about the DOM and other parts of the page.
pub struct Tab {
    target_id: TargetID,
    transport: Arc<Transport>,
    session_id: SessionId,
    navigating: Arc<AtomicBool>,
    target_info: Arc<Mutex<TargetInfo>>,
    request_interceptor: Arc<Mutex<Arc<RequestIntercept>>>,
    response_handler: Arc<Mutex<HashMap<String, ResponseHandler>>>,
    loading_failed_handler: Arc<Mutex<HashMap<String, LoadingFailedHandler>>>,
    auth_handler: Arc<Mutex<AuthChallengeResponse>>,
    default_wait: Arc<RwLock<Wait>>,
    page_bindings: Arc<Mutex<FunctionBinding>>,
    event_listeners: Arc<Mutex<Vec<Arc<SyncSendEvent>>>>,
    slow_motion_multiplier: Arc<RwLock<f64>>, // there's no AtomicF64, otherwise would use that
}

impl NoElementFound {
    pub fn map(error: Error) -> Error {
        match error.downcast::<RemoteError>() {
            Ok(remote_error) => {
                match remote_error.message.as_ref() {
                    // This error is expected and occurs while the page is still loading,
                    // hence we shadow it and respond the element is not found
                    "Could not find node with given id" => Self {}.into(),

                    // Any other error is unexpected and should be reported
                    _ => remote_error.into(),
                }
            },
            // Return original error if downcasting to RemoteError fails
            Err(original_error) => original_error,
        }
    }
}

impl Tab {
    pub async fn new(
        target_info: TargetInfo,
        transport: Arc<Transport>,
        timeout: Duration,
        sleep: Duration,
    ) -> Result<Self, ChromeError> {
        let target_id = target_info.target_id.clone();

        let session_id = transport
            .call_method_on_browser(AttachToTarget {
                target_id: target_id.clone(),
                flatten: None,
            })
            .await?
            .session_id
            .into();

        debug!("New tab attached with session ID: {:?}", session_id);

        let target_info_mutex = Arc::new(Mutex::new(target_info));

        let tab = Self {
            target_id,
            transport,
            session_id,
            navigating: Arc::new(AtomicBool::new(false)),
            target_info: target_info_mutex,
            page_bindings: Arc::new(Mutex::new(HashMap::new())),
            request_interceptor: Arc::new(Mutex::new(Arc::new(
                |_transport, _session_id, _interception| RequestPausedDecision::Continue(None),
            ))),
            response_handler: Arc::new(Mutex::new(HashMap::new())),
            loading_failed_handler: Arc::new(Mutex::new(HashMap::new())),
            auth_handler: Arc::new(Mutex::new(AuthChallengeResponse {
                response: Fetch::AuthChallengeResponseResponse::Default,
                username: None,
                password: None,
            })),
            default_wait: Arc::new(RwLock::new(Wait::new(timeout, sleep))),
            event_listeners: Arc::new(Mutex::new(Vec::new())),
            slow_motion_multiplier: Arc::new(RwLock::new(0.0)),
        };

        tab.call_method(Page::Enable(None))?;
        tab.call_method(Page::SetLifecycleEventsEnabled { enabled: true })?;

        tab.start_event_handler_thread();

        Ok(tab)
    }

    pub fn update_target_info(&self, target_info: TargetInfo) {
        let mut info = self.target_info.lock().unwrap();
        *info = target_info;
    }

    pub fn get_target_id(&self) -> &TargetID {
        &self.target_id
    }

    /// Fetches the most recent info about this target
    pub async fn get_target_info(&self) -> Result<TargetInfo, ChromeError> {
        Ok(self
            .call_method(Target::GetTargetInfo {
                target_id: Some(self.get_target_id().to_string()),
            })
            .await?
            .target_info)
    }

    pub async fn get_browser_context_id(&self) -> Result<Option<String>, ChromeError> {
        Ok(self.get_target_info().await?.browser_context_id)
    }

    pub fn get_url(&self) -> String {
        let info = self.target_info.lock().unwrap();
        info.url.clone()
    }

    /// Allows overriding user agent with the given string.
    pub async fn set_user_agent(
        &self,
        user_agent: &str,
        accept_language: Option<&str>,
        platform: Option<&str>,
    ) -> Result<(), ChromeError> {
        self.call_method(SetUserAgentOverride {
            user_agent: user_agent.to_string(),
            accept_language: accept_language.map(std::string::ToString::to_string),
            platform: platform.map(std::string::ToString::to_string),
            user_agent_metadata: None,
        })
        .await
        .map(|_| ())
    }

    async fn start_event_handler_thread(&self) {
        let transport: Arc<Transport> = Arc::clone(&self.transport);
        let incoming_events_rx = self
            .transport
            .listen_to_target_events(self.session_id.clone());
        let navigating = Arc::clone(&self.navigating);
        let interceptor_mutex = Arc::clone(&self.request_interceptor);
        let response_handler_mutex = self.response_handler.clone();
        let loading_failed_handler_mutex = self.loading_failed_handler.clone();
        let auth_handler_mutex = self.auth_handler.clone();
        let session_id = self.session_id.clone();
        let listeners_mutex = Arc::clone(&self.event_listeners);

        let bindings_mutex = Arc::clone(&self.page_bindings);
        let received_event_params = Arc::new(Mutex::new(HashMap::new()));

        thread::spawn(move || {
            for event in incoming_events_rx {
                let listeners = listeners_mutex.lock().unwrap();
                listeners.iter().for_each(|listener| {
                    listener.on_event(&event);
                });

                match event {
                    Event::PageLifecycleEvent(lifecycle_event) => {
                        let event_name = lifecycle_event.params.name.as_ref();
                        trace!("Lifecycle event: {}", event_name);
                        match event_name {
                            "networkAlmostIdle" => {
                                navigating.store(false, Ordering::SeqCst);
                            },
                            "init" => {
                                navigating.store(true, Ordering::SeqCst);
                            },
                            _ => {},
                        }
                    },
                    Event::RuntimeBindingCalled(binding) => {
                        let bindings = bindings_mutex.lock().unwrap().clone();

                        let name = binding.params.name;
                        let payload = binding.params.payload;

                        let func = &Arc::clone(bindings.get(&name).unwrap());

                        func.call_binding(json!(payload));
                    },
                    Event::FetchRequestPaused(event) => {
                        let interceptor = interceptor_mutex.lock().unwrap();
                        let decision = interceptor.intercept(
                            Arc::clone(&transport),
                            session_id.clone(),
                            event.clone(),
                        );
                        let result = match decision {
                            RequestPausedDecision::Continue(continue_request) => {
                                if let Some(continue_request) = continue_request {
                                    transport
                                        .call_method_on_target(session_id.clone(), continue_request)
                                        .await
                                        .map(|_| ())
                                } else {
                                    transport
                                        .call_method_on_target(
                                            session_id.clone(),
                                            ContinueRequest {
                                                request_id: event.params.request_id,
                                                url: None,
                                                method: None,
                                                post_data: None,
                                                headers: None,
                                                intercept_response: None,
                                            },
                                        )
                                        .await
                                        .map(|_| ())
                                }
                            },
                            RequestPausedDecision::Fulfill(fulfill_request) => transport
                                .call_method_on_target(session_id.clone(), fulfill_request)
                                .await
                                .map(|_| ()),
                            RequestPausedDecision::Fail(fail_request) => transport
                                .call_method_on_target(session_id.clone(), fail_request)
                                .await
                                .map(|_| ()),
                        };
                        if result.is_err() {
                            warn!("Tried to handle request after connection was closed");
                        }
                    },
                    Event::FetchAuthRequired(event) => {
                        let auth_challenge_response = auth_handler_mutex.lock().unwrap().clone();

                        let request_id = event.params.request_id;
                        let method = ContinueWithAuth {
                            request_id,
                            auth_challenge_response,
                        };
                        let result = transport
                            .call_method_on_target(session_id.clone(), method)
                            .await;
                        if result.is_err() {
                            warn!("Tried to handle request after connection was closed");
                        }
                    },
                    Event::NetworkResponseReceived(ev) => {
                        let request_id = ev.params.request_id.clone();
                        received_event_params
                            .lock()
                            .unwrap()
                            .insert(request_id, ev.params);
                    },
                    Event::NetworkLoadingFinished(ev) => {
                        response_handler_mutex.lock().unwrap().iter().for_each(
                            |(_name, handler)| {
                                let request_id = ev.params.request_id.clone();
                                let retrieve_body = || {
                                    let method = GetResponseBody {
                                        request_id: request_id.clone(),
                                    };
                                    transport.call_method_on_target(session_id.clone(), method)
                                };
                                if let Some(params) =
                                    received_event_params.lock().unwrap().get(&request_id)
                                {
                                    handler(params.clone(), &retrieve_body);
                                } else {
                                    warn!("Request id does not exist");
                                }
                            },
                        );
                    },
                    Event::NetworkLoadingFailed(ev) => loading_failed_handler_mutex
                        .lock()
                        .unwrap()
                        .iter()
                        .for_each(|(_name, handler)| {
                            let request_id = ev.params.request_id.clone();

                            if let Some(params) =
                                received_event_params.lock().unwrap().get(&request_id)
                            {
                                handler(params.clone(), ev.params.clone());
                            } else {
                                warn!("Request id does not exist");
                            }
                        }),
                    _ => {
                        let raw_event = format!("{event:?}");
                        trace!(
                            "Unhandled event: {}",
                            raw_event.chars().take(50).collect::<String>()
                        );
                    },
                }
            }
            info!("finished tab's event handling loop");
        });
    }

    pub fn expose_function(&self, name: &str, func: Arc<SafeBinding>) -> Result<(), ChromeError> {
        let bindings_mutex = Arc::clone(&self.page_bindings);

        let mut bindings = bindings_mutex.lock().unwrap();

        bindings.insert(name.to_string(), func);

        let expression = r#"
        (function addPageBinding(bindingName) {
            const binding = window[bindingName];
            window[bindingName] = (...args) => {
              const me = window[bindingName];
              let callbacks = me['callbacks'];
              if (!callbacks) {
                callbacks = new Map();
                me['callbacks'] = callbacks;
              }
              const seq = (me['lastSeq'] || 0) + 1;
              me['lastSeq'] = seq;
              const promise = new Promise((resolve, reject) => callbacks.set(seq, {resolve, reject}));
              binding(JSON.stringify({name: bindingName, seq, args}));
              return promise;
            };
          })()
        "#; // https://github.com/puppeteer/puppeteer/blob/97c9fe2520723d45a5a86da06b888ae888d400be/src/common/helper.ts#L183

        self.call_method(AddBinding {
            name: name.to_string(),
            execution_context_id: None,
            execution_context_name: None,
        })?;

        self.call_method(AddScriptToEvaluateOnNewDocument {
            source: expression.to_string(),
            world_name: None,
            include_command_line_api: None,
        })?;

        Ok(())
    }

    pub fn remove_function(&self, name: &str) -> Result<(), ChromeError> {
        let bindings_mutex = Arc::clone(&self.page_bindings);

        let mut bindings = bindings_mutex.lock().unwrap();

        bindings.remove(name).unwrap();

        Ok(())
    }

    pub async fn call_method<C>(&self, method: C) -> Result<C::ReturnObject, ChromeError>
    where
        C: Method + serde::Serialize + std::fmt::Debug,
    {
        trace!("Calling method: {:?}", method);
        let result = self
            .transport
            .call_method_on_target(self.session_id.clone(), method)
            .await;
        let result_string = format!("{result:?}");
        trace!("Got result: {:?}", result_string.chars().take(70));
        result
    }

    pub async fn wait_until_navigated(&self) -> Result<&Self, ChromeError> {
        let navigating = Arc::clone(&self.navigating);
        let wait = *self.default_wait.read().unwrap();

        wait.until(|| {
            if navigating.load(Ordering::SeqCst) {
                None
            } else {
                Some(true)
            }
        })
        .await?;
        debug!("A tab finished navigating");

        Ok(self)
    }

    // Pulls focus to this tab
    pub async fn bring_to_front(&self) -> Result<Page::BringToFrontReturnObject, ChromeError> {
        self.call_method(Page::BringToFront(None)).await
    }

    pub async fn navigate_to(&self, url: &str) -> Result<&Self, ChromeError> {
        let return_object = self
            .call_method(Navigate {
                url: url.to_string(),
                referrer: None,
                transition_Type: None,
                frame_id: None,
                referrer_policy: None,
            })
            .await?;
        if let Some(error_text) = return_object.error_text {
            return Err(ChromeError::NavigationFailed(error_text));
        }

        let navigating = Arc::clone(&self.navigating);
        navigating.store(true, Ordering::SeqCst);

        info!("Navigating a tab to {}", url);

        Ok(self)
    }

    /// Set default wait for the tab
    ///
    /// This will be applied to all [wait_for_element](Tab::wait_for_element) and [wait_for_elements](Tab::wait_for_elements) calls for this tab
    pub fn set_default_wait(&self, timeout: Duration, sleep: Duration) -> &Self {
        let mut current_wait = self.default_wait.write().unwrap();
        *current_wait = Wait::new(timeout, sleep);
        self
    }

    /// Analogous to Puppeteer's ['slowMo' option](https://github.com/GoogleChrome/puppeteer/blob/v1.20.0/docs/api.md#puppeteerconnectoptions),
    /// but with some differences:
    ///
    /// * It doesn't add a delay after literally every message sent via the protocol, but instead
    ///   just for:
    ///     * clicking a specific point on the page (default: 100ms before moving the mouse, 250ms
    ///       before pressing and releasting mouse button)
    ///     * pressing a key (default: 25 ms)
    ///     * reloading the page (default: 100ms)
    ///     * closing a tab (default: 100ms)
    /// * Instead of an absolute number of milliseconds, it's a multiplier, so that we can delay
    ///   longer on certain actions like clicking or moving the mouse, and shorter on others like
    ///   on pressing a key (or the individual 'mouseDown' and 'mouseUp' actions that go across the
    ///   wire. If the delay was always the same, filling out a form (e.g.) would take ages).
    ///
    /// By default the multiplier is set to zero, which effectively disables the slow motion.
    ///
    /// The defaults for the various actions (i.e. how long we sleep for when
    /// multiplier is 1.0) are supposed to be just slow enough to help a human see what's going on
    /// as a test runs.
    pub fn set_slow_motion_multiplier(&self, multiplier: f64) -> &Self {
        let mut slow_motion_multiplier = self.slow_motion_multiplier.write().unwrap();
        *slow_motion_multiplier = multiplier;
        self
    }

    async fn optional_slow_motion_sleep(&self, millis: u64) {
        let multiplier = self.slow_motion_multiplier.read().unwrap();
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let scaled_millis = millis * *multiplier as u64;
        sleep(Duration::from_millis(scaled_millis));
    }

    pub async fn wait_for_element(
        &self,
        selector: &str,
        wait: Wait,
    ) -> Result<Element<'_>, ChromeError> {
        debug!("Waiting for element with selector: {:?}", selector);
        wait.strict_until(
            || self.find_element(selector),
            Error::downcast::<NoElementFound>,
        )
        .await
    }

    pub async fn wait_for_xpath(
        &self,
        selector: &str,
        wait: Wait,
    ) -> Result<Element<'_>, ChromeError> {
        debug!("Waiting for element with selector: {:?}", selector);
        wait.strict_until(
            || self.find_element_by_xpath(selector),
            Error::downcast::<NoElementFound>,
        )
        .await
    }

    pub async fn wait_for_elements(&self, selector: &str) -> Result<Vec<Element<'_>>, ChromeError> {
        debug!("Waiting for element with selector: {:?}", selector);
        *self.default_wait.read().unwrap().strict_until(
            || self.find_elements(selector),
            Error::downcast::<NoElementFound>,
        )
    }

    pub async fn wait_for_elements_by_xpath(
        &self,
        selector: &str,
    ) -> Result<Vec<Element<'_>>, ChromeError> {
        debug!("Waiting for element with selector: {:?}", selector);
        *self.default_wait.read().unwrap().strict_until(
            || self.find_elements_by_xpath(selector),
            Error::downcast::<NoElementFound>,
        )
    }

    /// Returns the first element in the document which matches the given selector.
    ///
    /// Equivalent to the following JS:
    ///
    /// ```js
    /// document.querySelector(selector)
    /// ```
    pub async fn find_element(&self, selector: &str) -> Result<Element<'_>, ChromeError> {
        let root_node_id = self.get_document().await?.node_id;
        trace!("Looking up element via selector: {}", selector);

        self.run_query_selector_on_node(root_node_id, selector)
            .await
    }

    pub async fn find_element_by_xpath(&self, query: &str) -> Result<Element<'_>, ChromeError> {
        self.get_document().await?;

        self.call_method(DOM::PerformSearch {
            query: query.to_string(),
            include_user_agent_shadow_dom: None,
        })
        .await
        .and_then(|o| {
            Ok(self
                .call_method(DOM::GetSearchResults {
                    search_id: o.search_id,
                    from_index: 0,
                    to_index: o.result_count,
                })
                .await?
                .node_ids[0])
        })
        .and_then(|id| {
            if id == 0 {
                Err(ChromeError::NoElementFound)
            } else {
                Ok(Element::new(self, id)?)
            }
        })
    }

    pub async fn run_query_selector_on_node(
        &self,
        node_id: NodeId,
        selector: &str,
    ) -> Result<Element<'_>, ChromeError> {
        let node_id = self
            .call_method(DOM::QuerySelector {
                node_id,
                selector: selector.to_string(),
            })
            .await?
            .node_id;

        Element::new(self, node_id)
    }

    pub async fn run_query_selector_all_on_node(
        &self,
        node_id: NodeId,
        selector: &str,
    ) -> Result<Vec<Element<'_>>, ChromeError> {
        let node_ids = self
            .call_method(DOM::QuerySelectorAll {
                node_id,
                selector: selector.to_string(),
            })
            .await?
            .node_ids;

        node_ids
            .iter()
            .map(|node_id| Element::new(self, *node_id))
            .collect()
    }

    pub async fn get_document(&self) -> Result<Node, ChromeError> {
        Ok(self
            .call_method(DOM::GetDocument {
                depth: Some(0),
                pierce: Some(false),
            })
            .await?
            .root)
    }

    /// Get the full HTML contents of the page.
    pub async fn get_content(&self) -> Result<String, ChromeError> {
        let func = "
            (function () {
                let retVal = '';
                if (document.doctype)
                    retVal = new XMLSerializer().serializeToString(document.doctype);
                if (document.documentElement)
                    retVal += document.documentElement.outerHTML;
                return retVal;
            })();";
        let html = self.evaluate(func, false).await?.value.unwrap();
        Ok(String::from(html.as_str().unwrap()))
    }

    pub async fn find_elements(&self, selector: &str) -> Result<Vec<Element<'_>>> {
        trace!("Looking up elements via selector: {}", selector);

        let root_node_id = self.get_document().await?.node_id;
        let node_ids = self
            .call_method(DOM::QuerySelectorAll {
                node_id: root_node_id,
                selector: selector.to_string(),
            })
            .await?
            .node_ids;

        if node_ids.is_empty() {
            return Err(NoElementFound {}.into());
        }

        node_ids
            .into_iter()
            .map(|node_id| Element::new(self, node_id))
            .collect()
    }

    pub async fn find_elements_by_xpath(
        &self,
        query: &str,
    ) -> Result<Vec<Element<'_>>, ChromeError> {
        self.get_document()?;

        self.call_method(DOM::PerformSearch {
            query: query.to_string(),
            include_user_agent_shadow_dom: None,
        })
        .await
        .and_then(|o| {
            Ok(self
                .call_method(DOM::GetSearchResults {
                    search_id: o.search_id,
                    from_index: 0,
                    to_index: o.result_count,
                })
                .await?
                .node_ids)
        })
        .and_then(|ids| {
            ids.iter()
                .filter(|id| **id != 0)
                .map(|id| Element::new(self, *id))
                .collect()
        })
    }

    pub async fn describe_node(&self, node_id: NodeId) -> Result<Node, ChromeError> {
        let node = self
            .call_method(DOM::DescribeNode {
                node_id: Some(node_id),
                backend_node_id: None,
                depth: Some(100),
                object_id: None,
                pierce: None,
            })
            .await?
            .node;
        Ok(node)
    }

    pub async fn type_str(&self, string_to_type: &str) -> Result<&Self, ChromeError> {
        for c in string_to_type.split("") {
            // split call above will have empty string at start and end which we won't type
            if c.is_empty() {
                continue;
            }
            let definition = keys::get_key_definition(c);
            // https://github.com/puppeteer/puppeteer/blob/b8806d5625ca7835abbaf2e997b0bf35a5679e29/src/common/Input.ts#L239-L245
            match definition {
                Ok(key) => {
                    let v: DispatchKeyEvent = key.into();

                    self.call_method(v.clone()).await?;
                    self.call_method(DispatchKeyEvent {
                        Type: Input::DispatchKeyEventTypeOption::KeyUp,
                        ..v
                    })
                    .await?;
                },
                Err(_) => {
                    self.send_character(c).await?;
                },
            }
        }
        Ok(self)
    }

    /// Does the same as `type_str` but it only dispatches a `keypress` and `input` event.
    /// It does not send a `keydown` or `keyup` event.
    ///
    /// What this means is that it is much faster.
    /// It is especially useful when you have a lot of text as input.
    pub async fn send_character(&self, char_to_send: &str) -> Result<&Self, ChromeError> {
        self.call_method(Input::InsertText {
            text: char_to_send.to_string(),
        })
        .await?;
        Ok(self)
    }

    pub async fn press_key(
        &self,
        key: &str,
        slow_motion_sleep: Option<u64>,
    ) -> Result<&Self, ChromeError> {
        // See https://github.com/GoogleChrome/puppeteer/blob/62da2366c65b335751896afbb0206f23c61436f1/lib/Input.js#L114-L115
        let definiton = keys::get_key_definition(key)?;

        let text = definiton
            .text
            .or({
                if definiton.key.len() == 1 {
                    Some(definiton.key)
                } else {
                    None
                }
            })
            .map(ToString::to_string);

        // See https://github.com/GoogleChrome/puppeteer/blob/62da2366c65b335751896afbb0206f23c61436f1/lib/Input.js#L52
        let key_down_event_type = if text.is_some() {
            Input::DispatchKeyEventTypeOption::KeyDown
        } else {
            Input::DispatchKeyEventTypeOption::RawKeyDown
        };

        let key = Some(definiton.key.to_string());
        let code = Some(definiton.code.to_string());

        if let Some(slow_motion_sleep) = slow_motion_sleep {
            self.optional_slow_motion_sleep(slow_motion_sleep).await;
        }

        self.call_method(Input::DispatchKeyEvent {
            Type: key_down_event_type,
            key: key.clone(),
            text: text.clone(),
            code: code.clone(),
            windows_virtual_key_code: Some(definiton.key_code),
            native_virtual_key_code: Some(definiton.key_code),
            modifiers: None,
            timestamp: None,
            unmodified_text: None,
            key_identifier: None,
            auto_repeat: None,
            is_keypad: None,
            is_system_key: None,
            location: None,
            commands: None,
        })?;
        self.call_method(Input::DispatchKeyEvent {
            Type: Input::DispatchKeyEventTypeOption::KeyUp,
            key,
            text,
            code,
            windows_virtual_key_code: Some(definiton.key_code),
            native_virtual_key_code: Some(definiton.key_code),
            modifiers: None,
            timestamp: None,
            unmodified_text: None,
            key_identifier: None,
            auto_repeat: None,
            is_keypad: None,
            is_system_key: None,
            location: None,
            commands: None,
        })?;
        Ok(self)
    }

    /// Moves the mouse to this point (dispatches a mouseMoved event)
    pub fn move_mouse_to_point(
        &self,
        point: Point,
        slow_motions_sleep: Option<u64>,
    ) -> Result<&Self, ChromeError> {
        if point.x == 0.0 && point.y == 0.0 {
            warn!("Midpoint of element shouldn't be 0,0. Something is probably wrong.");
        }

        if let Some(slow_motion_sleep) = slow_motion_sleep {
            self.optional_slow_motion_sleep(slow_motion_sleep).await;
        }

        self.call_method(Input::DispatchMouseEvent {
            Type: Input::DispatchMouseEventTypeOption::MouseMoved,
            x: point.x,
            y: point.y,
            modifiers: None,
            timestamp: None,
            button: None,
            buttons: None,
            click_count: None,
            force: None,
            tangential_pressure: None,
            tilt_x: None,
            tilt_y: None,
            twist: None,
            delta_x: None,
            delta_y: None,
            pointer_Type: None,
        })?;

        Ok(self)
    }

    pub async fn click_point(
        &self,
        point: Point,
        slow_motion_sleep: Option<u64>,
    ) -> Result<&Self, ChromeError> {
        trace!("Clicking point: {:?}", point);
        if point.x == 0.0 && point.y == 0.0 {
            warn!("Midpoint of element shouldn't be 0,0. Something is probably wrong.");
        }

        self.move_mouse_to_point(point)?;

        if let Some(slow_motion_sleep) = slow_motion_sleep {
            self.optional_slow_motion_sleep(slow_motion_sleep).await;
        }

        self.call_method(Input::DispatchMouseEvent {
            Type: Input::DispatchMouseEventTypeOption::MousePressed,
            x: point.x,
            y: point.y,
            button: Some(Input::MouseButton::Left),
            click_count: Some(1),
            modifiers: None,
            timestamp: None,
            buttons: None,
            force: None,
            tangential_pressure: None,
            tilt_x: None,
            tilt_y: None,
            twist: None,
            delta_x: None,
            delta_y: None,
            pointer_Type: None,
        })?;
        self.call_method(Input::DispatchMouseEvent {
            Type: Input::DispatchMouseEventTypeOption::MouseReleased,
            x: point.x,
            y: point.y,
            button: Some(Input::MouseButton::Left),
            click_count: Some(1),
            modifiers: None,
            timestamp: None,
            buttons: None,
            force: None,
            tangential_pressure: None,
            tilt_x: None,
            tilt_y: None,
            twist: None,
            delta_x: None,
            delta_y: None,
            pointer_Type: None,
        })?;
        Ok(self)
    }

    /// Capture a screenshot of the current page.
    ///
    /// If `clip` is given, the screenshot is taken of the specified region only.
    /// `Element::get_box_model()` can be used to get regions of certains elements
    /// on the page; there is also `Element::capture_screenhot()` as a shorthand.
    ///
    /// If `from_surface` is true, the screenshot is taken from the surface rather than
    /// the view.
    pub fn capture_screenshot(
        &self,
        format: Page::CaptureScreenshotFormatOption,
        quality: Option<u32>,
        clip: Option<Page::Viewport>,
        from_surface: bool,
    ) -> Result<Vec<u8>, ChromeError> {
        let data = self
            .call_method(Page::CaptureScreenshot {
                format: Some(format),
                clip,
                quality,
                from_surface: Some(from_surface),
                capture_beyond_viewport: None,
            })?
            .data;
        base64::prelude::BASE64_STANDARD
            .decode(data)
            .map_err(Into::into)
    }

    pub fn print_to_pdf(&self, options: Option<PrintToPdfOptions>) -> Result<Vec<u8>, ChromeError> {
        if let Some(options) = options {
            let transfer_mode: Option<Page::PrintToPDFTransfer_modeOption> =
                options.transfer_mode.and_then(std::convert::Into::into);
            let data = self
                .call_method(Page::PrintToPDF {
                    landscape: options.landscape,
                    display_header_footer: options.display_header_footer,
                    print_background: options.print_background,
                    scale: options.scale,
                    paper_width: options.paper_width,
                    paper_height: options.paper_height,
                    margin_top: options.margin_top,
                    margin_bottom: options.margin_bottom,
                    margin_left: options.margin_left,
                    margin_right: options.margin_right,
                    page_ranges: options.page_ranges,
                    ignore_invalid_page_ranges: options.ignore_invalid_page_ranges,
                    header_template: options.header_template,
                    footer_template: options.footer_template,
                    prefer_css_page_size: options.prefer_css_page_size,
                    transfer_mode,
                })?
                .data;
            base64::prelude::BASE64_STANDARD
                .decode(data)
                .map_err(Into::into)
        } else {
            let data = self
                .call_method(Page::PrintToPDF {
                    ..Default::default()
                })?
                .data;

            base64::prelude::BASE64_STANDARD
                .decode(data)
                .map_err(Into::into)
        }
    }

    /// Reloads given page optionally ignoring the cache
    ///
    /// If `ignore_cache` is true, the browser cache is ignored (as if the user pressed Shift+F5).
    /// If `script_to_evaluate` is given, the script will be injected into all frames of the
    /// inspected page after reload. Argument will be ignored if reloading dataURL origin.
    pub async fn reload(
        &self,
        ignore_cache: bool,
        script_to_evaluate_on_load: Option<&str>,
        slow_motion_sleep: Option<u64>,
    ) -> Result<&Self, ChromeError> {
        if let Some(slow_motion_sleep) = slow_motion_sleep {
            self.optional_slow_motion_sleep(slow_motion_sleep).await;
        }

        self.call_method(Page::Reload {
            ignore_cache: Some(ignore_cache),
            script_to_evaluate_on_load: script_to_evaluate_on_load
                .map(std::string::ToString::to_string),
        })
        .await?;
        Ok(self)
    }

    /// Set the background color of the dom to transparent.
    ///
    /// Useful when you want capture a .png
    pub fn set_transparent_background_color(&self) -> Result<&Self, ChromeError> {
        self.call_method(Emulation::SetDefaultBackgroundColorOverride {
            color: Some(DOM::RGBA {
                r: 0,
                g: 0,
                b: 0,
                a: Some(0.0),
            }),
        })?;
        Ok(self)
    }

    /// Set the default background color of the dom.
    ///
    /// Pass a RGBA to override the backrgound color of the dom.
    pub fn set_background_color(&self, color: DOM::RGBA) -> Result<&Self, ChromeError> {
        self.call_method(Emulation::SetDefaultBackgroundColorOverride { color: Some(color) })?;
        Ok(self)
    }

    /// Enables the profiler
    pub fn enable_profiler(&self) -> Result<&Self, ChromeError> {
        self.call_method(Profiler::Enable(None))?;

        Ok(self)
    }

    /// Disables the profiler
    pub fn disable_profiler(&self) -> Result<&Self, ChromeError> {
        self.call_method(Profiler::Disable(None))?;

        Ok(self)
    }

    /// Starts tracking which lines of JS have been executed
    ///
    /// Will return error unless `enable_profiler` has been called.
    ///
    /// Equivalent to hitting the record button in the "coverage" tab in Chrome DevTools.
    /// See the file `tests/coverage.rs` for an example.
    ///
    /// By default we enable the 'detailed' flag on StartPreciseCoverage, which enables block-level
    /// granularity, and also enable 'call_count' (which when disabled always sets count to 1 or 0).
    ///
    pub fn start_js_coverage(&self) -> Result<&Self, ChromeError> {
        self.call_method(Profiler::StartPreciseCoverage {
            call_count: Some(true),
            detailed: Some(true),
            allow_triggered_updates: None,
        })?;
        Ok(self)
    }

    /// Stops tracking which lines of JS have been executed
    /// If you're finished with the profiler, don't forget to call `disable_profiler`.
    pub fn stop_js_coverage(&self) -> Result<&Self, ChromeError> {
        self.call_method(Profiler::StopPreciseCoverage(None))?;
        Ok(self)
    }

    /// Collect coverage data for the current isolate, and resets execution counters.
    ///
    /// Precise code coverage needs to have started (see `start_js_coverage`).
    ///
    /// Will only send information about code that's been executed since this method was last
    /// called, or (if this is the first time) since calling `start_js_coverage`.
    /// Another way of thinking about it is: every time you call this, the call counts for
    /// FunctionRanges are reset after returning.
    ///
    /// The format of the data is a little unintuitive, see here for details:
    /// <https://chromedevtools.github.io/devtools-protocol/tot/Profiler#type-ScriptCoverage>
    pub fn take_precise_js_coverage(&self) -> Result<Vec<Profiler::ScriptCoverage>, ChromeError> {
        let script_coverages = self
            .call_method(Profiler::TakePreciseCoverage(None))?
            .result;
        Ok(script_coverages)
    }

    /// Enables fetch domain.
    pub fn enable_fetch(
        &self,
        patterns: Option<&[Fetch::RequestPattern]>,
        handle_auth_requests: Option<bool>,
    ) -> Result<&Self, ChromeError> {
        self.call_method(Fetch::Enable {
            patterns: patterns.map(Vec::from),
            handle_auth_requests,
        })?;
        Ok(self)
    }

    /// Disables fetch domain
    pub fn disable_fetch(&self) -> Result<&Self, ChromeError> {
        self.call_method(Fetch::Disable(None))?;
        Ok(self)
    }

    /// Allows you to inspect outgoing network requests from the tab, and optionally return
    /// your own responses to them
    ///
    /// The `interceptor` argument is a closure which takes this tab's `Transport` and its SessionID
    /// so that you can call methods from within the closure using `transport.call_method_on_target`.
    ///
    /// The closure needs to return a variant of `RequestPausedDecision`.
    pub fn enable_request_interception(
        &self,
        interceptor: Arc<RequestIntercept>,
    ) -> Result<(), ChromeError> {
        let mut current_interceptor = self.request_interceptor.lock().unwrap();
        *current_interceptor = interceptor;
        Ok(())
    }

    pub fn authenticate(
        &self,
        username: Option<String>,
        password: Option<String>,
    ) -> Result<&Self, ChromeError> {
        let mut current_auth_handler = self.auth_handler.lock().unwrap();
        *current_auth_handler = AuthChallengeResponse {
            response: Fetch::AuthChallengeResponseResponse::ProvideCredentials,
            username,
            password,
        };
        Ok(self)
    }

    /// Lets you register a listener for responses, and gives you a way to get the response body too.
    ///
    /// Please note that the 'response' does not include the *body* of the response -- Chrome tells
    /// us about them seperately (because you might quickly get the status code and headers from a
    /// server well before you receive the entire response body which could, after all, be gigabytes
    /// long).
    ///
    /// Currently we leave it up to the caller to decide when to call `fetch_body` (the second
    /// argument to the response handler), although ideally it wouldn't be possible until Chrome has
    /// sent the `Network.loadingFinished` event.
    ///
    /// Return a option for ResponseHander for existing handler with same name if existed.
    pub fn register_response_handling<S: ToString>(
        &self,
        handler_name: S,
        handler: ResponseHandler,
    ) -> Result<Option<ResponseHandler>, ChromeError> {
        self.call_method(Network::Enable {
            max_total_buffer_size: None,
            max_resource_buffer_size: None,
            max_post_data_size: None,
        })?;
        Ok(self
            .response_handler
            .lock()
            .unwrap()
            .insert(handler_name.to_string(), handler))
    }

    pub fn register_loading_failed_handling<S: ToString>(
        &self,
        handler_name: S,
        handler: LoadingFailedHandler,
    ) -> Result<Option<LoadingFailedHandler>, ChromeError> {
        self.call_method(Network::Enable {
            max_total_buffer_size: None,
            max_resource_buffer_size: None,
            max_post_data_size: None,
        })?;
        Ok(self
            .loading_failed_handler
            .lock()
            .unwrap()
            .insert(handler_name.to_string(), handler))
    }

    /// Deregister a reponse handler based on its name.
    ///
    /// Return a option for ResponseHandler for removed handler if existed.
    pub fn deregister_response_handling(
        &self,
        handler_name: &str,
    ) -> Result<Option<ResponseHandler>, ChromeError> {
        Ok(self.response_handler.lock().unwrap().remove(handler_name))
    }

    /// Deregister all registered handlers.
    pub fn deregister_response_handling_all(&self) -> Result<(), ChromeError> {
        self.response_handler.lock().unwrap().clear();
        Ok(())
    }

    /// Enables runtime domain.
    pub fn enable_runtime(&self) -> Result<&Self, ChromeError> {
        self.call_method(Runtime::Enable(None))?;
        Ok(self)
    }

    /// Disables runtime domain
    pub fn disable_runtime(&self) -> Result<&Self, ChromeError> {
        self.call_method(Runtime::Disable(None))?;
        Ok(self)
    }

    /// Enables Debugger
    pub fn enable_debugger(&self) -> Result<(), ChromeError> {
        self.call_method(Debugger::Enable {
            max_scripts_cache_size: None,
        })?;
        Ok(())
    }

    /// Disables Debugger
    pub fn disable_debugger(&self) -> Result<(), ChromeError> {
        self.call_method(Debugger::Disable(None))?;
        Ok(())
    }

    /// Returns source for the script with given id.
    ///
    /// Debugger must be enabled.
    pub fn get_script_source(&self, script_id: &str) -> Result<String, ChromeError> {
        Ok(self
            .call_method(Debugger::GetScriptSource {
                script_id: script_id.to_string(),
            })?
            .script_source)
    }

    /// Enables log domain.
    ///
    /// Sends the entries collected so far to the client by means of the entryAdded notification.
    ///
    /// See <https://chromedevtools.github.io/devtools-protocol/tot/Log#method-enable>
    pub fn enable_log(&self) -> Result<&Self, ChromeError> {
        self.call_method(Log::Enable(None))?;

        Ok(self)
    }

    /// Disables log domain
    ///
    /// Prevents further log entries from being reported to the client
    ///
    /// See <https://chromedevtools.github.io/devtools-protocol/tot/Log#method-disable>
    pub fn disable_log(&self) -> Result<&Self, ChromeError> {
        self.call_method(Log::Disable(None))?;

        Ok(self)
    }

    /// Starts violation reporting
    ///
    /// See <https://chromedevtools.github.io/devtools-protocol/tot/Log#method-startViolationsReport>
    pub fn start_violations_report(
        &self,
        config: Vec<ViolationSetting>,
    ) -> Result<&Self, ChromeError> {
        self.call_method(Log::StartViolationsReport { config })?;
        Ok(self)
    }

    /// Stop violation reporting
    ///
    /// See <https://chromedevtools.github.io/devtools-protocol/tot/Log#method-stopViolationsReport>
    pub fn stop_violations_report(&self) -> Result<&Self, ChromeError> {
        self.call_method(Log::StopViolationsReport(None))?;
        Ok(self)
    }

    /// Evaluates expression on global object.
    pub async fn evaluate(
        &self,
        expression: &str,
        await_promise: bool,
    ) -> Result<Runtime::RemoteObject, ChromeError> {
        let result = self
            .call_method(Runtime::Evaluate {
                expression: expression.to_string(),
                return_by_value: Some(false),
                generate_preview: Some(true),
                silent: Some(false),
                await_promise: Some(await_promise),
                include_command_line_api: Some(false),
                user_gesture: Some(false),
                object_group: None,
                context_id: None,
                throw_on_side_effect: None,
                timeout: None,
                disable_breaks: None,
                repl_mode: None,
                allow_unsafe_eval_blocked_by_csp: None,
                unique_context_id: None,
            })
            .await?
            .result;
        Ok(result)
    }

    /// Adds event listener to Event
    ///
    /// Make sure you are enabled domain you are listening events to.
    pub fn add_event_listener(
        &self,
        listener: Arc<SyncSendEvent>,
    ) -> Result<Weak<SyncSendEvent>, ChromeError> {
        let mut listeners = self.event_listeners.lock().unwrap();
        listeners.push(listener);
        Ok(Arc::downgrade(listeners.last().unwrap()))
    }

    pub fn remove_event_listener(&self, listener: &Weak<SyncSendEvent>) -> Result<(), ChromeError> {
        let listener = listener.upgrade();
        if listener.is_none() {
            return Ok(());
        }
        let listener = listener.unwrap();
        let mut listeners = self.event_listeners.lock().unwrap();
        let pos = listeners.iter().position(|x| Arc::ptr_eq(x, &listener));
        if let Some(idx) = pos {
            listeners.remove(idx);
        }

        Ok(())
    }

    /// Closes the target Page
    pub async fn close_target(&self) -> Result<bool, ChromeError> {
        self.call_method(Target::CloseTarget {
            target_id: self.get_target_id().to_string(),
        })
        .await
        .map(|r| r.success)
    }

    /// Tries to close page, running its beforeunload hooks, if any
    pub async fn close_with_unload(&self) -> Result<bool, ChromeError> {
        self.call_method(Page::Close(None)).await.map(|_| true)
    }

    /// Calls one of the close_* methods depending on fire_unload option
    pub async fn close(
        &self,
        fire_unload: bool,
        slow_motion_sleep: Option<u64>,
    ) -> Result<bool, ChromeError> {
        if let Some(slow_motion_sleep) = slow_motion_sleep {
            self.optional_slow_motion_sleep(slow_motion_sleep).await;
        }

        if fire_unload {
            return self.close_with_unload().await;
        }
        self.close_target().await
    }

    /// Activates (focuses) the target.
    pub async fn activate(&self) -> Result<&Self, ChromeError> {
        self.call_method(Target::ActivateTarget {
            target_id: self.get_target_id().clone(),
        })
        .await
        .map(|_| self)
    }

    /// Get position and size of the browser window associated with this `Tab`.
    ///
    /// Note that the returned bounds are always specified for normal (windowed)
    /// state; they do not change when minimizing, maximizing or setting to
    /// fullscreen.
    pub async fn get_bounds(&self) -> Result<CurrentBounds, ChromeError> {
        self.transport
            .call_method_on_browser(Browser::GetWindowForTarget {
                target_id: Some(self.get_target_id().to_string()),
            })
            .await
            .map(|r| r.bounds.into())
    }

    /// Set position and/or size of the browser window associated with this `Tab`.
    ///
    /// When setting the window to normal (windowed) state, unspecified fields
    /// are left unchanged.
    pub async fn set_bounds(&self, bounds: Bounds) -> Result<&Self, ChromeError> {
        let window_id = self
            .transport
            .call_method_on_browser(Browser::GetWindowForTarget {
                target_id: Some(self.get_target_id().to_string()),
            })
            .await?
            .window_id;
        // If we set Normal window state, we *have* to make two API calls
        // to set the state before setting the coordinates; despite what the docs say...
        if let Bounds::Normal { .. } = &bounds {
            self.transport
                .call_method_on_browser(Browser::SetWindowBounds {
                    window_id,
                    bounds: Browser::Bounds {
                        left: None,
                        top: None,
                        width: None,
                        height: None,
                        window_state: Some(Browser::WindowState::Normal),
                    },
                })
                .await?;
        }
        self.transport
            .call_method_on_browser(Browser::SetWindowBounds {
                window_id,
                bounds: bounds.into(),
            })
            .await?;
        Ok(self)
    }

    /// Returns all cookies that match the tab's current URL.
    pub async fn get_cookies(&self) -> Result<Vec<Cookie>, ChromeError> {
        Ok(self
            .call_method(Network::GetCookies { urls: None })
            .await?
            .cookies)
    }

    /// Set cookies with tab's current URL
    pub async fn set_cookies(&self, cs: Vec<Network::CookieParam>) -> Result<(), ChromeError> {
        // puppeteer 7b24e5435b:src/common/Page.ts :1009-1028
        use Network::SetCookies;
        let url = self.get_url();
        let starts_with_http = url.starts_with("http");
        let cookies: Vec<Network::CookieParam> = cs
            .into_iter()
            .map(|c| {
                if c.url.is_none() && starts_with_http {
                    Network::CookieParam {
                        url: Some(url.clone()),
                        ..c
                    }
                } else {
                    c
                }
            })
            .collect();
        self.delete_cookies(
            cookies
                .clone()
                .into_iter()
                .map(std::convert::Into::into)
                .collect(),
        )
        .await?;
        self.call_method(SetCookies { cookies }).await?;
        Ok(())
    }

    /// Delete cookies with tab's current URL
    pub async fn delete_cookies(&self, cs: Vec<Network::DeleteCookies>) -> Result<(), ChromeError> {
        // puppeteer 7b24e5435b:src/common/Page.ts :998-1007
        let url = self.get_url();
        let starts_with_http = url.starts_with("http");
        cs.into_iter()
            .map(|c| {
                // REVIEW: if c.url is blank string
                if c.url.is_none() && starts_with_http {
                    Network::DeleteCookies {
                        url: Some(url.clone()),
                        ..c
                    }
                } else {
                    c
                }
            })
            .try_for_each(|c| -> Result<(), anyhow::Error> {
                let _ = self.call_method(c).await?;
                Ok(())
            })?;
        Ok(())
    }

    /// Returns the title of the document.
    pub async fn get_title(&self) -> Result<String, ChromeError> {
        let remote_object = self.evaluate("document.title", false).await?;
        Ok(serde_json::from_value(remote_object.value.unwrap())?)
    }

    /// If enabled, instead of using the GUI to select files, the browser will
    /// wait for the `Tab.handle_file_chooser` method to be called.
    /// **WARNING**: Only works on Chromium / Chrome 77 and above.
    pub async fn set_file_chooser_dialog_interception(&self, enabled: bool) -> Result<()> {
        self.call_method(SetInterceptFileChooserDialog { enabled })
            .await?;
        Ok(())
    }

    /// Will have the same effect as choosing these files from the file chooser dialog that would've
    /// popped up had `set_file_chooser_dialog_interception` not been called. Calls to this method
    /// must be preceded by calls to that method.
    ///
    /// Supports selecting files or closing the file chooser dialog.
    ///
    /// NOTE: the filepaths listed in `files` must be absolute.
    pub async fn handle_file_chooser(
        &self,
        files: Vec<String>,
        node_id: u32,
    ) -> Result<(), ChromeError> {
        self.call_method(DOM::SetFileInputFiles {
            files,
            node_id: Some(node_id),
            backend_node_id: None,
            object_id: None,
        })
        .await?;
        Ok(())
    }

    pub async fn set_extra_http_headers(
        &self,
        headers: HashMap<&str, &str>,
    ) -> Result<(), ChromeError> {
        self.call_method(Network::Enable {
            max_total_buffer_size: None,
            max_resource_buffer_size: None,
            max_post_data_size: None,
        })
        .await?;
        self.call_method(SetExtraHTTPHeaders {
            headers: Network::Headers(Some(json!(headers))),
        })
        .await?;
        Ok(())
    }

    pub async fn set_storage<T>(&self, item_name: &str, item: T) -> Result<(), ChromeError>
    where
        T: Serialize,
    {
        let value = json!(item).to_string();

        self.evaluate(
            &format!(r#"localStorage.setItem("{item_name}",JSON.stringify({value}))"#),
            false,
        )
        .await?;

        Ok(())
    }

    pub async fn get_storage<T>(&self, item_name: &str) -> Result<T, ChromeError>
    where
        T: DeserializeOwned,
    {
        let object = self
            .evaluate(&format!(r#"localStorage.getItem("{item_name}")"#), false)
            .await?;

        let json: Option<T> = object.value.and_then(|v| match v {
            serde_json::Value::String(ref s) => {
                let result = serde_json::from_str(s);

                if let Ok(r) = result {
                    Some(r)
                } else {
                    Some(serde_json::from_value(v).unwrap())
                }
            },
            _ => None,
        });

        match json {
            Some(v) => Ok(v),
            None => Err(NoLocalStorageItemFound {}.into()),
        }
    }

    pub async fn remove_storage(&self, item_name: &str) -> Result<(), ChromeError> {
        self.evaluate(&format!(r#"localStorage.removeItem("{item_name}")"#), false)
            .await?;

        Ok(())
    }

    pub async fn stop_loading(&self) -> Result<bool, ChromeError> {
        self.call_method(Page::StopLoading(None))
            .await
            .map(|_| true)
    }

    async fn bypass_user_agent(&self) -> Result<(), ChromeError> {
        let object = self.evaluate("window.navigator.userAgent", true).await?;

        match object.value.map(|x| x.to_string()) {
            Some(mut ua) => {
                ua = ua.replace("HeadlessChrome/", "Chrome/");

                let re = regex::Regex::new(r"\(([^)]+)\)")?;
                ua = re.replace(&ua, "(Windows NT 10.0; Win64; x64)").to_string();

                self.set_user_agent(&ua, None, None)?;
                Ok(())
            },
            None => Err(ChromeError::NoUserAgentEvaluated),
        }
    }

    async fn bypass_wedriver(&self) -> Result<(), ChromeError> {
        self.call_method(Page::AddScriptToEvaluateOnNewDocument {
            source: "Object.defineProperty(navigator, 'webdriver', {get: () => undefined});"
                .to_string(),
            world_name: None,
            include_command_line_api: None,
        })
        .await?;
        Ok(())
    }

    async fn bypass_chrome(&self) -> Result<(), ChromeError> {
        self.call_method(Page::AddScriptToEvaluateOnNewDocument {
            source: "window.chrome = { runtime: {} };".to_string(),
            world_name: None,
            include_command_line_api: None,
        })
        .await?;
        Ok(())
    }

    async fn bypass_permissions(&self) -> Result<(), ChromeError> {
        let r = "const originalQuery = window.navigator.permissions.query;
        window.navigator.permissions.__proto__.query = parameters =>
        parameters.name === 'notifications'
            ? Promise.resolve({state: Notification.permission})
            : originalQuery(parameters);";

        self.call_method(Page::AddScriptToEvaluateOnNewDocument {
            source: r.to_string(),
            world_name: None,
            include_command_line_api: None,
        })
        .await?;
        Ok(())
    }

    async fn bypass_plugins(&self) -> Result<(), ChromeError> {
        self.call_method(Page::AddScriptToEvaluateOnNewDocument {
            source: "Object.defineProperty(navigator, 'plugins', { get: () => [
            {filename:'internal-pdf-viewer'},
            {filename:'adsfkjlkjhalkh'},
            {filename:'internal-nacl-plugin'}
          ], });"
                .to_string(),
            world_name: None,
            include_command_line_api: None,
        })
        .await?;
        Ok(())
    }

    async fn bypass_webgl_vendor(&self) -> Result<(), ChromeError> {
        let r = "const getParameter = WebGLRenderingContext.getParameter;
        WebGLRenderingContext.prototype.getParameter = function(parameter) {
            // UNMASKED_VENDOR_WEBGL
            if (parameter === 37445) {
                return 'Google Inc. (NVIDIA)';
            }
            // UNMASKED_RENDERER_WEBGL
            if (parameter === 37446) {
                return 'ANGLE (NVIDIA, NVIDIA GeForce GTX 1050 Direct3D11 vs_5_0 ps_5_0, D3D11-27.21.14.5671)';
            }

            return getParameter(parameter);
        };";

        self.call_method(Page::AddScriptToEvaluateOnNewDocument {
            source: r.to_string(),
            world_name: None,
            include_command_line_api: None,
        })
        .await?;
        Ok(())
    }

    pub async fn enable_stealth_mode(&self) -> Result<(), ChromeError> {
        self.bypass_user_agent().await?;
        self.bypass_wedriver().await?;
        self.bypass_chrome().await?;
        self.bypass_permissions().await?;
        self.bypass_plugins().await?;
        self.bypass_webgl_vendor().await?;
        Ok(())
    }
}
