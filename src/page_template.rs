use askama::Template;

#[derive(Debug, Template)]
#[template(path = "page.html")]
pub struct PageTemplate {
    pub title: String,
    pub body: String,
    pub use_websocket: bool,
}
