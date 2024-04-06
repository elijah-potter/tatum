use askama::Template;

#[derive(Debug, Template)]
#[template(path = "text.svg")]
pub struct SvgTemplate {
    pub fill: String,
    pub text: String,
}
