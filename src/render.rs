use crate::page_template::PageTemplate;
use askama::Template;
use base64::{engine::general_purpose, Engine};
use pulldown_cmark::{Event, LinkType, Tag};
use resolve_path::PathResolveExt;
use std::path::{Path, PathBuf};
use url::Url;

use tokio::fs::{read, read_to_string};

use crate::svg_template::SvgTemplate;

fn data_url(data: &[u8], mime_type: &str) -> String {
    let encoded = general_purpose::STANDARD.encode(data);

    format!("data:{};base64,{encoded}", mime_type)
}

/// Gets the file at a specified path, loads it, and converts it to a base64-encoded data URL
async fn path_to_data_url(path: impl AsRef<Path>) -> anyhow::Result<String> {
    let file = read(&path).await?;

    Ok(data_url(
        &file,
        mime_guess::from_path(&path)
            .first_raw()
            .unwrap_or("text/plain"),
    ))
}

/// Generates an SVG image containing a message and serializes it to a data URL.
fn generate_message_data_url(message: impl AsRef<str>, color: impl AsRef<str>) -> String {
    data_url(
        SvgTemplate {
            fill: color.as_ref().to_string(),
            text: message.as_ref().to_string(),
        }
        .to_string()
        .as_bytes(),
        "image/svg+xml",
    )
}

/// Renders a file to an HTML string.
///
/// `use_websocket` determines whether to include code for automatically updating the document with a
/// WebSocket connection.
pub async fn render_doc(path: impl AsRef<Path>, use_websocket: bool) -> anyhow::Result<String> {
    let path = path.as_ref().canonicalize()?;

    let file = read_to_string(&path).await?;

    let options = pulldown_cmark::Options::all();

    let parser = pulldown_cmark::Parser::new_ext(file.as_str(), options);

    let mut events: Vec<_> = parser.collect();

    // Handle URLs
    for event in events.iter_mut() {
        // Resolve image links asynchronously
        if let Event::Start(Tag::Image {
            link_type: LinkType::Inline,
            dest_url,
            ..
        }) = event
        {
            if dest_url.parse::<Url>().is_ok() {
                continue;
            } else if let Ok(image_path) = dest_url.parse::<PathBuf>() {
                *dest_url = path_to_data_url(image_path.resolve_in(&path))
                    .await
                    .unwrap_or(generate_message_data_url("Disk error.", "red"))
                    .into()
                // Tag::Link
            } else {
                *dest_url = generate_message_data_url("Unable to parse image path.", "red").into();
            }
        }

        // Rewrite URLs to open links
        if let Event::Start(Tag::Link {
            link_type: LinkType::Inline,
            dest_url,
            ..
        }) = event
        {
            // If the link is a valid URL, leave it
            if !dest_url.parse::<Url>().is_ok() {
                // Otherwise, try to parse it as a file path
                if let Ok(file_path) = dest_url.parse::<PathBuf>() {
                    // Rewrite the URL to open correctly
                    if let Some(file_path) = file_path.to_str() {
                        // If it's a relative path, resolve it
                        let file_path = if PathBuf::from(file_path).is_relative() {
                            rel_to_abspath(file_path, path.clone())
                        } else {
                            file_path.to_string()
                        };
                        // Write the URL
                        *dest_url = format!("/?path={}", file_path).into()
                    }
                }
            }
        }
    }

    let mut body = String::new();
    pulldown_cmark::html::push_html(&mut body, events.into_iter());

    let template = PageTemplate {
        body,
        title: path.as_os_str().to_string_lossy().to_string(),
        use_websocket,
    };

    Ok(template.render().unwrap())
}

/// Converts a relative file path to an absolute path using a reference file path.
///
/// This function takes a relative file path and a current absolute file path,
/// it returns the absolute path of the target file. It does not panic.
///
/// Note: The `current_file` input must correspond to a file path, not a directory path.
///
/// # Arguments
///
/// * `path` - A str of the relative file path to be converted.
/// * `current_file` - A PathBuf which holds the path to the current file.
///
/// # Return
///
/// This function returns a String that represents the absolute path of the target file.
///
/// ```
/// let current_file = PathBuf::from("/home/user/Notes/slipbox/networking/dns.md");
/// assert_eq!(rel_to_abspath("../linux.md", current_file), String::from("/home/user/Notes/slipbox/linux.md"));
/// ```
///
fn rel_to_abspath(path: &str, current_file: PathBuf) -> String {
    let current_dir = current_file
        .parent()
        .unwrap_or_else(|| &current_file)
        .to_path_buf()
        .join(path);

    // Clean up the path
    let mut clean_path = PathBuf::new();
    for component in current_dir.components() {
        match component {
            std::path::Component::ParentDir => {
                clean_path.pop();
            }
            std::path::Component::CurDir => continue,
            _ => clean_path.push(component),
        }
    }

    clean_path.to_str().unwrap_or_else(|| path).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rel_to_abspath() {
        let current_file = PathBuf::from("/home/user/Notes/slipbox/networking/dns.md");
        assert_eq!(
            rel_to_abspath("../linux.md", current_file.clone(),),
            String::from("/home/user/Notes/slipbox/linux.md")
        );

        let current_dir = current_file.parent().unwrap().to_path_buf();
        println!("{:?} is a directory: {}", current_dir, current_dir.is_dir());

        // If passed a directory, we expect a mistaken link as the function
        // gets the directory with pop and requires the directory
        // to exist in order to test
        assert_eq!(
            rel_to_abspath("../linux.md", current_dir),
            String::from("/home/user/Notes/linux.md")
        );
    }
}
