//! DOM wiring. The document in `index.html` is the page; this module fills it.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Document, Element, EventSource, Headers, HtmlButtonElement, HtmlElement, HtmlFormElement,
    HtmlInputElement, MessageEvent, Request, RequestInit, RequestMode, Response, SubmitEvent,
    Window,
};

use crate::location::query_param;
use crate::reduce::{ChatMessage, View, apply_server_message, note_trace};
use crate::wire::{
    Command, CommandEnvelope, CommandResult, MOCK_MODEL, SteerMode, decode_server_message,
};

struct Page {
    view: View,
    base: String,
    connection: String,
    next_id: u32,
    last_result: String,
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    let window = web_sys::window().expect("window");
    let page = Rc::new(RefCell::new(Page {
        view: View::default(),
        base: daemon_base(&window),
        connection: "connecting".into(),
        next_id: 0,
        last_result: String::new(),
    }));
    page.borrow().render();
    bind_form(&page);
    bind_stream(&page);
}

fn daemon_base(window: &Window) -> String {
    let location = window.location();
    let search = location.search().unwrap_or_default();
    if let Some(daemon) = query_param(&search, "daemon") {
        let trimmed = daemon.trim_end_matches('/').to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }
    location
        .origin()
        .unwrap_or_else(|_| "http://127.0.0.1:8788".into())
}

fn bind_form(page: &Rc<RefCell<Page>>) {
    let form = element("composer")
        .dyn_into::<HtmlFormElement>()
        .expect("#composer is a form");
    let page_for_send = Rc::clone(page);
    let on_submit = Closure::<dyn FnMut(SubmitEvent)>::new(move |event: SubmitEvent| {
        event.prevent_default();
        let page = Rc::clone(&page_for_send);
        wasm_bindgen_futures::spawn_local(async move {
            submit(&page, None).await;
        });
    });
    form.add_event_listener_with_callback("submit", on_submit.as_ref().unchecked_ref())
        .expect("submit listener");
    on_submit.forget();

    listen_click("steer", page, |page| async move {
        submit(&page, Some(SteerMode::Steer)).await;
    });
    listen_click("cancel", page, |page| async move {
        abort(&page).await;
    });
}

fn listen_click<F, Fut>(id: &str, page: &Rc<RefCell<Page>>, action: F)
where
    F: Fn(Rc<RefCell<Page>>) -> Fut + 'static,
    Fut: std::future::Future<Output = ()> + 'static,
{
    let button = element(id);
    let page = Rc::clone(page);
    let on_click = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event: web_sys::Event| {
        let page = Rc::clone(&page);
        wasm_bindgen_futures::spawn_local(action(page));
    });
    button
        .add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref())
        .expect("click listener");
    on_click.forget();
}

fn bind_stream(page: &Rc<RefCell<Page>>) {
    let url = format!("{}/events", page.borrow().base);
    let source = EventSource::new(&url).unwrap_or_else(|_| panic!("event source {url}"));

    let page_for_open = Rc::clone(page);
    let on_open = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event: web_sys::Event| {
        set_connection(&page_for_open, "live");
    });
    source.set_onopen(Some(on_open.as_ref().unchecked_ref()));
    on_open.forget();

    let page_for_error = Rc::clone(page);
    let source_for_error = source.clone();
    let on_error = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event: web_sys::Event| {
        let connection = if source_for_error.ready_state() == EventSource::CLOSED {
            "closed"
        } else {
            "reconnecting"
        };
        set_connection(&page_for_error, connection);
    });
    source.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    on_error.forget();

    let page_for_message = Rc::clone(page);
    let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        let Some(data) = event.data().as_string() else {
            return;
        };
        let mut page = page_for_message.borrow_mut();
        match decode_server_message(&data) {
            Ok(message) => apply_server_message(&mut page.view, &message),
            Err(err) => note_trace(&mut page.view, format!("bad frame · {err}")),
        }
        page.render();
    });
    source.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    on_message.forget();
    std::mem::forget(source);
}

fn set_connection(page: &Rc<RefCell<Page>>, connection: &str) {
    let mut page = page.borrow_mut();
    if page.connection == connection {
        return;
    }
    page.connection = connection.to_string();
    web_sys::console::log_1(&JsValue::from_str(&page.connection));
    page.render();
}

async fn submit(page: &Rc<RefCell<Page>>, when_busy: Option<SteerMode>) {
    let envelope = {
        let mut page = page.borrow_mut();
        let text = input_value().trim().to_string();
        if text.is_empty() {
            return;
        }
        clear_input();
        page.next_id += 1;
        let id = page.next_id;
        CommandEnvelope {
            id: format!("chat-{id}"),
            command: Command::Submit {
                conversation_id: page.view.conversation_id.clone(),
                text,
                request_id: Some(format!("req-{id}")),
                when_busy,
            },
        }
    };
    deliver(page, envelope).await;
}

async fn abort(page: &Rc<RefCell<Page>>) {
    let envelope = {
        let mut page = page.borrow_mut();
        page.next_id += 1;
        let id = page.next_id;
        CommandEnvelope {
            id: format!("chat-{id}"),
            command: Command::Abort {
                conversation_id: page.view.conversation_id.clone(),
            },
        }
    };
    deliver(page, envelope).await;
}

async fn deliver(page: &Rc<RefCell<Page>>, envelope: CommandEnvelope) {
    let base = page.borrow().base.clone();
    let id = envelope.id.clone();
    let line = match post_command(&base, &envelope).await {
        Ok(result) => result_line(&result),
        Err(err) => format!("{id} · {err}"),
    };
    let mut page = page.borrow_mut();
    page.last_result = line.clone();
    note_trace(&mut page.view, line);
    page.render();
}

async fn post_command(base: &str, envelope: &CommandEnvelope) -> Result<CommandResult, String> {
    let window = web_sys::window().ok_or("window")?;
    let json = serde_json::to_string(envelope).map_err(|err| err.to_string())?;
    let headers = Headers::new().map_err(|_| "headers")?;
    headers
        .set("content-type", "application/json")
        .map_err(|_| "content-type")?;
    headers
        .set("accept", "application/json")
        .map_err(|_| "accept")?;
    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_mode(RequestMode::Cors);
    opts.set_headers(&headers);
    opts.set_body(&JsValue::from_str(&json));
    let url = format!("{base}/commands");
    let request = Request::new_with_str_and_init(&url, &opts).map_err(|_| "request")?;
    let value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|_| format!("fetch {url} failed"))?;
    let response: Response = value.dyn_into().map_err(|_| "response")?;
    let text_value = JsFuture::from(response.text().map_err(|_| "body")?)
        .await
        .map_err(|_| "read body".to_string())?;
    let text = text_value.as_string().unwrap_or_default();
    serde_json::from_str(&text).map_err(|err| format!("{err}: {text}"))
}

fn result_line(result: &CommandResult) -> String {
    match &result.error {
        Some(error) => format!(
            "{} · {} · {}",
            result.id,
            error.code.as_deref().unwrap_or("error"),
            error.message
        ),
        None => format!("{} · ack", result.id),
    }
}

impl Page {
    fn render(&self) {
        let document = document();
        let status = element("status");
        let connection = self.connection.as_str();
        let _ = status.set_attribute("data-state", connection);
        status.set_text_content(Some(&status_text(self)));
        element("banner").set_text_content(Some(&banner(&self.view)));
        element("conversation").set_text_content(Some(&conversation_line(&self.view)));
        element("inbox").set_text_content(Some(&inbox_line(&self.view)));
        element("last-result").set_text_content(Some(&self.last_result));
        render_messages(&document, &self.view.messages);
        render_trace(&document, &self.view.trace);
        if let Some(button) = element("cancel").dyn_ref::<HtmlButtonElement>() {
            button.set_disabled(!self.view.running);
        }
    }
}

fn status_text(page: &Page) -> String {
    let run = if page.view.running {
        "running"
    } else if let Some(status) = &page.view.run_status {
        status.as_str()
    } else {
        "idle"
    };
    format!("{} · {run}", page.connection)
}

fn banner(view: &View) -> String {
    match view.agent_model.as_deref() {
        Some(MOCK_MODEL) => {
            "Mock server. Not the icedrop daemon. It accepts submit and abort on POST /commands."
                .into()
        }
        Some(model) => format!("Connected. Agent model: {model}"),
        None => "Test surface for POST /commands and GET /events.".into(),
    }
}

fn conversation_line(view: &View) -> String {
    view.conversation_id
        .as_ref()
        .map(|id| format!("conversation {id}"))
        .unwrap_or_default()
}

fn inbox_line(view: &View) -> String {
    if view.inbox.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = view
        .inbox
        .iter()
        .map(|item| {
            format!(
                "{} {}",
                item.mode.as_str(),
                item.text.as_deref().unwrap_or("")
            )
        })
        .collect();
    format!("Queued: {}", parts.join(" · "))
}

fn render_messages(document: &Document, messages: &[ChatMessage]) {
    let list = element("messages");
    clear(&list);
    if messages.is_empty() {
        let item = document.create_element("li").expect("li");
        item.set_class_name("empty");
        item.set_text_content(Some("No messages yet. Send starts a run."));
        let _ = list.append_child(&item);
        return;
    }
    for message in messages {
        let item = document.create_element("li").expect("li");
        let class = if message.live {
            format!("{} live", message.role)
        } else {
            message.role.clone()
        };
        item.set_class_name(&class);
        let role = document.create_element("span").expect("span");
        role.set_class_name("role");
        let label = if message.live {
            format!("{} · streaming", message.role)
        } else {
            message.role.clone()
        };
        role.set_text_content(Some(&label));
        let body = document.create_element("p").expect("p");
        body.set_text_content(Some(&message.text));
        let _ = item.append_child(&role);
        let _ = item.append_child(&body);
        let _ = list.append_child(&item);
    }
    scroll_to_end(&list);
}

fn render_trace(document: &Document, trace: &[String]) {
    let list = element("events");
    clear(&list);
    for line in trace {
        let item = document.create_element("li").expect("li");
        item.set_text_content(Some(line));
        let _ = list.append_child(&item);
    }
    scroll_to_end(&list);
}

fn scroll_to_end(list: &Element) {
    if let Some(html) = list.dyn_ref::<HtmlElement>() {
        html.set_scroll_top(html.scroll_height());
    }
}

fn clear(node: &Element) {
    while let Some(child) = node.first_child() {
        let _ = node.remove_child(&child);
    }
}

fn input_value() -> String {
    element("text")
        .dyn_into::<HtmlInputElement>()
        .map(|input| input.value())
        .unwrap_or_default()
}

fn clear_input() {
    if let Ok(input) = element("text").dyn_into::<HtmlInputElement>() {
        input.set_value("");
    }
}

fn document() -> Document {
    web_sys::window()
        .expect("window")
        .document()
        .expect("document")
}

fn element(id: &str) -> Element {
    document()
        .get_element_by_id(id)
        .unwrap_or_else(|| panic!("#{id} missing from index.html"))
}
