use std::collections::HashMap;
use std::fmt::Debug;
use std::time::Duration;

use anyhow::{Error, Result};

use log::{debug, error};

use crate::protocol::cdp::Runtime::RemoteObject;
use crate::{browser::tab::point::Point, protocol::cdp::CSS::CSSComputedStyleProperty};

mod box_model;

use crate::{wait, ChromeError};
pub use box_model::{BoxModel, ElementQuad};

use crate::protocol::cdp::{Page, Runtime, CSS, DOM};

/// A handle to a [DOM Element](https://developer.mozilla.org/en-US/docs/Web/API/Element).
///
/// Typically you get access to these by passing `Tab.wait_for_element` a CSS selector. Once
/// you have a handle to an element, you can click it, type into it, inspect its
/// attributes, and more. You can even run a JavaScript function inside the tab which can reference
/// the element via `this`.
pub struct Element<'a> {
    pub remote_object_id: String,
    pub backend_node_id: DOM::NodeId,
    pub node_id: DOM::NodeId,
    pub parent: &'a super::Tab,
    pub attributes: Option<Vec<String>>,
    pub tag_name: String,
    pub value: String,
}

impl<'a> Debug for Element<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "Element {}", self.backend_node_id)?;
        Ok(())
    }
}

impl<'a> Element<'a> {
    /// Using a 'node_id', of the type returned by QuerySelector and QuerySelectorAll, this finds
    /// the 'backend_node_id' and 'remote_object_id' which are stable identifiers, unlike node_id.
    /// We use these two when making various calls to the API because of that.
    pub async fn new(parent: &'a super::Tab, node_id: DOM::NodeId) -> Result<Self, ChromeError> {
        if node_id == 0 {
            return Err(ChromeError::NoElementFound);
        }

        let node = parent
            .describe_node(node_id)
            .await
            .map_err(ChromeError::NoElementFound)?;

        let attributes = node.attributes;
        let tag_name = node.node_name;

        let backend_node_id = node.backend_node_id;

        let object = parent
            .call_method(DOM::ResolveNode {
                backend_node_id: Some(backend_node_id),
                node_id: None,
                object_group: None,
                execution_context_id: None,
            })
            .await?
            .object;

        let value = object.value.unwrap_or("".into()).to_string();
        let remote_object_id = object.object_id.expect("couldn't find object ID");

        Ok(Element {
            remote_object_id,
            backend_node_id,
            node_id,
            parent,
            attributes,
            tag_name,
            value,
        })
    }

    /// Returns the first element in the document which matches the given CSS selector.
    ///
    /// Equivalent to the following JS:
    ///
    /// ```js
    /// document.querySelector(selector)
    /// ```
    pub async fn find_element(&self, selector: &str) -> Result<Self, ChromeError> {
        self.parent
            .run_query_selector_on_node(self.node_id, selector)
            .await
    }

    pub async fn find_element_by_xpath(&self, query: &str) -> Result<Element<'_>, ChromeError> {
        self.parent.get_document()?;

        self.parent
            .call_method(DOM::PerformSearch {
                query: query.to_string(),
                include_user_agent_shadow_dom: Some(true),
            })
            .await
            .and_then(|o| {
                Ok(self
                    .parent
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
                    Ok(Element::new(self.parent, id)?)
                }
            })
    }

    /// Returns the first element in the document which matches the given CSS selector.
    ///
    /// Equivalent to the following JS:
    ///
    /// ```js
    /// document.querySelector(selector)
    /// ```
    pub async fn find_elements(&self, selector: &str) -> Result<Vec<Self>, ChromeError> {
        self.parent
            .run_query_selector_all_on_node(self.node_id, selector)
            .await
    }

    pub async fn find_elements_by_xpath(&self, query: &str) -> Result<Vec<Element<'_>>> {
        self.parent.get_document()?;
        self.parent
            .call_method(DOM::PerformSearch {
                query: query.to_string(),
                include_user_agent_shadow_dom: Some(true),
            })
            .await
            .and_then(|o| {
                Ok(self
                    .parent
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
                    .map(|id| Element::new(self.parent, *id))
                    .collect()
            })
    }

    pub async fn wait_for_element(
        &self,
        selector: &str,
        timeout: Duration,
        sleep: Duration,
    ) -> Result<Element<'_>, ChromeError> {
        debug!("Waiting for element with selector: {:?}", selector);
        wait::Wait::new(timeout, sleep).strict_until(
            || self.find_element(selector).await,
            Error::downcast::<NoElementFound>,
        )
    }

    pub async fn wait_for_xpath(
        &self,
        selector: &str,
        timeout: Duration,
        sleep: Duration,
    ) -> Result<Element<'_>, ChromeError> {
        debug!("Waiting for element with selector: {:?}", selector);
        wait::Wait::new(timeout, sleep).strict_until(
            || self.find_element_by_xpath(selector).await,
            Error::downcast::<NoElementFound>,
        )
    }

    pub fn wait_for_elements(
        &self,
        selector: &str,
        timeout: Duration,
        sleep: Duration,
    ) -> Result<Vec<Element<'_>>, ChromeError> {
        debug!("Waiting for element with selector: {:?}", selector);
        wait::Wait::new(timeout, sleep).strict_until(
            || self.find_elements(selector).await,
            Error::downcast::<NoElementFound>,
        )
    }

    pub async fn wait_for_elements_by_xpath(
        &self,
        selector: &str,
        timeout: Duration,
        sleep: Duration,
    ) -> Result<Vec<Element<'_>>, ChromeError> {
        debug!("Waiting for element with selector: {:?}", selector);
        wait::Wait::new(timeout, sleep).strict_until(
            || self.find_elements_by_xpath(selector),
            Error::downcast::<NoElementFound>,
        )
    }

    /// Moves the mouse to the middle of this element
    pub async fn move_mouse_over(&self) -> Result<&Self, ChromeError> {
        self.scroll_into_view().await?;
        let midpoint = self.get_midpoint().await?;
        self.parent.move_mouse_to_point(midpoint).await?;
        Ok(self)
    }

    pub async fn click(&self) -> Result<&Self, ChromeError> {
        self.scroll_into_view().await?;
        debug!("Clicking element {:?}", &self);
        let midpoint = self.get_midpoint().await?;
        self.parent.click_point(midpoint).await?;
        Ok(self)
    }

    pub async fn type_into(&self, text: &str) -> Result<&Self, ChromeError> {
        self.click().await?;

        debug!("Typing into element ( {:?} ): {}", &self, text);

        self.parent.type_str(text).await?;

        Ok(self)
    }

    pub async fn call_js_fn(
        &self,
        function_declaration: &str,
        args: Vec<serde_json::Value>,
        await_promise: bool,
    ) -> Result<Runtime::RemoteObject, ChromeError> {
        let mut args = args;
        let result = self
            .parent
            .call_method(Runtime::CallFunctionOn {
                object_id: Some(self.remote_object_id.clone()),
                function_declaration: function_declaration.to_string(),
                arguments: args
                    .iter_mut()
                    .map(|v| {
                        Some(Runtime::CallArgument {
                            value: Some(v.take()),
                            unserializable_value: None,
                            object_id: None,
                        })
                    })
                    .collect(),
                return_by_value: Some(false),
                generate_preview: Some(true),
                silent: Some(false),
                await_promise: Some(await_promise),
                user_gesture: None,
                execution_context_id: None,
                object_group: None,
                throw_on_side_effect: None,
            })
            .await?
            .result;

        Ok(result)
    }

    pub async fn focus(&self) -> Result<&Self, ChromeError> {
        self.scroll_into_view().await?;
        self.parent
            .call_method(DOM::Focus {
                backend_node_id: Some(self.backend_node_id),
                node_id: None,
                object_id: None,
            })
            .await?;
        Ok(self)
    }

    /// Returns the inner text of an HTML Element. Returns an empty string on elements with no text.
    ///
    /// Note: .innerText and .textContent are not the same thing. See:
    /// <https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/innerText>
    ///
    /// Note: if you somehow call this on a node that's not an HTML Element (e.g. `document`), this
    /// will fail.
    pub async fn get_inner_text(&self) -> Result<String, ChromeError> {
        let text: String = serde_json::from_value(
            self.call_js_fn("function() { return this.innerText }", vec![], false)
                .await?
                .value
                .unwrap(),
        )?;
        Ok(text)
    }

    /// Get the full HTML contents of the element.
    ///
    /// Equivalent to the following JS: ```element.outerHTML```.
    pub async fn get_content(&self) -> Result<String, ChromeError> {
        let html = self
            .call_js_fn("function() { return this.outerHTML }", vec![], false)
            .await?
            .value
            .unwrap();

        Ok(String::from(html.as_str().unwrap()))
    }

    pub async fn get_computed_styles(&self) -> Result<Vec<CSSComputedStyleProperty>, ChromeError> {
        let styles = self
            .parent
            .call_method(CSS::GetComputedStyleForNode {
                node_id: self.node_id,
            })
            .await?
            .computed_style;

        Ok(styles)
    }

    pub async fn get_description(&self) -> Result<DOM::Node, ChromeError> {
        let node = self
            .parent
            .call_method(DOM::DescribeNode {
                node_id: None,
                backend_node_id: Some(self.backend_node_id),
                depth: Some(100),
                object_id: None,
                pierce: None,
            })
            .await?
            .node;
        Ok(node)
    }

    /// Capture a screenshot of this element.
    ///
    /// The screenshot is taken from the surface using this element's content-box.
    pub async fn capture_screenshot(
        &self,
        format: Page::CaptureScreenshotFormatOption,
    ) -> Result<Vec<u8>, ChromeError> {
        self.scroll_into_view().await?;
        self.parent
            .capture_screenshot(
                format,
                Some(90),
                Some(self.get_box_model().await?.content_viewport().await),
                true,
            )
            .await
    }

    pub async fn set_input_files(&self, file_paths: &[&str]) -> Result<&Self, ChromeError> {
        self.parent
            .call_method(DOM::SetFileInputFiles {
                files: file_paths
                    .to_vec()
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect(),
                backend_node_id: Some(self.backend_node_id),
                node_id: None,
                object_id: None,
            })
            .await?;
        Ok(self)
    }

    /// Scrolls the current element into view
    ///
    /// Used prior to any action applied to the current element to ensure action is duable.
    pub async fn scroll_into_view(&self) -> Result<&Self, ChromeError> {
        let result = self
            .call_js_fn(
                "async function() {
                if (!this.isConnected)
                    return 'Node is detached from document';
                if (this.nodeType !== Node.ELEMENT_NODE)
                    return 'Node is not of type HTMLElement';

                const visibleRatio = await new Promise(resolve => {
                    const observer = new IntersectionObserver(entries => {
                        resolve(entries[0].intersectionRatio);
                        observer.disconnect();
                    });
                    observer.observe(this);
                });

                if (visibleRatio !== 1.0)
                    this.scrollIntoView({
                        block: 'center',
                        inline: 'center',
                        behavior: 'instant'
                    });
                return false;
            }",
                vec![],
                true,
            )
            .await?;

        if result.Type == Runtime::RemoteObjectType::String {
            let error_text = result.value.unwrap().as_str().unwrap().to_string();
            return Err(ChromeError::ScrollFailed(error_text));
        }

        Ok(self)
    }

    pub async fn get_attributes(&self) -> Result<Option<Vec<String>>, ChromeError> {
        let description = self.get_description().await?;
        Ok(description.attributes)
    }

    /// Get boxes for this element
    pub async fn get_box_model(&self) -> Result<BoxModel, ChromeError> {
        let model = self
            .parent
            .call_method(DOM::GetBoxModel {
                node_id: None,
                backend_node_id: Some(self.backend_node_id),
                object_id: None,
            })
            .await?
            .model;
        Ok(BoxModel {
            content: ElementQuad::from_raw_points(&model.content),
            padding: ElementQuad::from_raw_points(&model.padding),
            border: ElementQuad::from_raw_points(&model.border),
            margin: ElementQuad::from_raw_points(&model.margin),
            width: model.width as f64,
            height: model.height as f64,
        })
    }

    pub async fn get_midpoint(
        &self,
        timeout: Duration,
        sleep: Duration,
    ) -> Result<Point, ChromeError> {
        if let Ok(e) = self
            .parent
            .call_method(DOM::GetContentQuads {
                node_id: None,
                backend_node_id: Some(self.backend_node_id),
                object_id: None,
            })
            .await
            .map(|quad| {
                let raw_quad = quad.quads.first().unwrap();
                let input_quad = ElementQuad::from_raw_points(raw_quad);

                (input_quad.bottom_right + input_quad.top_left) / 2.0
            })
        {
            return Ok(e);
        }
        // let mut p = Point { x: 0.0, y: 0.0 }; FIX FOR CLIPPY `value assigned to `p` is never read`
        let p = wait::Wait::new(timeout, sleep).until(|| {
            let r = self
                .call_js_fn(
                    r#"
                    function() {
                        let rect = this.getBoundingClientRect();

                        if(rect.x != 0) {
                            this.scrollIntoView();
                        }

                        return this.getBoundingClientRect();
                    }
                    "#,
                    vec![],
                    false,
                )
                .await
                .unwrap();

            let res = extract_midpoint(r);

            match res {
                Ok(v) => {
                    if v.x == 0.0 {
                        None
                    } else {
                        Some(v)
                    }
                },
                _ => None,
            }
        })?;

        Ok(p)
    }

    pub async fn get_js_midpoint(&self) -> Result<Point, ChromeError> {
        let result = self
            .call_js_fn(
                "function(){return this.getBoundingClientRect(); }",
                vec![],
                false,
            )
            .await?;

        extract_midpoint(result)
    }
}

fn extract_midpoint(remote_obj: RemoteObject) -> Result<Point, ChromeError> {
    let mut prop_map = HashMap::new();

    match remote_obj.preview.map(|v| {
        for prop in v.properties {
            prop_map.insert(prop.name, prop.value.unwrap().parse::<f64>().unwrap());
        }
        Point {
            x: prop_map["x"] + (prop_map["width"] / 2.0),
            y: prop_map["y"] + (prop_map["height"] / 2.0),
        }
    }) {
        Some(v) => Ok(v),
        None => Ok(Point { x: 0.0, y: 0.0 }),
    }
}
