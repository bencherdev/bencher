mod provider;

use log::{debug, info};
use provider::{Provider, Tick};
use std::rc::Rc;
use wasm_bindgen::JsValue;
use yew::prelude::*;
use yewtil::future::LinkFuture;

mod bindings;

enum Msg {
    FetchChart,
    SetFetchChartResult(Vec<Tick>),
    ShowError(String),
}

struct Model {
    link: ComponentLink<Self>,
    provider: Rc<Provider>,
    error_msg: String,
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Self {
            link,
            provider: Rc::new(Provider {}),
            error_msg: "".to_owned(),
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::FetchChart => {
                debug!("Fetching chart...");
                let provider = self.provider.clone();
                self.link.send_future(async move {
                    match provider.fetch_chart().await {
                        Ok(entries) => Msg::SetFetchChartResult(entries),
                        Err(err) => Msg::ShowError(format!("{}", err)),
                    }
                });
                false
            }
            Msg::SetFetchChartResult(chart) => {
                Self::show_chart(chart);
                true
            }
            Msg::ShowError(msg) => {
                info!("Error: {:?}", msg);
                self.error_msg = msg;
                true
            }
        }
    }

    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        false
    }

    fn view(&self) -> Html {
        html! {
            <div>
                <button onclick=self.link.callback(|_| Msg::FetchChart)>{ "Fetch" }</button>
                <p style="color: red;"> { self.error_msg.clone() }</p>
                <svg id="chart"></svg>
            </div>
        }
    }
}

impl Model {
    fn show_chart(ticks: Vec<Tick>) {
        debug!("Showing chart");
        // call js
        // the bindings are defined in bindings.rs
        bindings::show_chart(JsValue::from_serde(&ticks).unwrap());
    }
}

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::start_app::<Model>();
}
