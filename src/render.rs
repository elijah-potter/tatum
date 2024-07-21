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
                    // If it's a filepath check if it's relative
                    if let Some(file_path) = file_path.to_str() {
                        let file_path = if Path::new(file_path).is_relative() {
                            // If it's relative, join it to the current file
                            join_and_canonicalize(&file_path, path.clone())
                        } else {
                            // Otherwise, use the file path as is
                            file_path.into()
                        };

                        // If possible, return a relative path from the cwd
                        let file_path = match get_relative_path_under_cwd(file_path.clone()) {
                            Some(path) => path,
                            None => file_path,
                        };
                        *dest_url = format!("/?path={}", file_path.to_str().unwrap()).into()
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

/// Returns a relative path to a file if it is under the working directory
///
/// # Arguments
/// * `file_path` - A `PathBuf` representing the file or directory path to check and convert if necessary.
///
/// # Returns
/// * A `PathBuf` object containing either a relative or absolute path to the `file_path`.
///
/// # Examples
/// ```
/// use std::path::PathBuf;
/// let file_path = std::env::current_dir().unwrap().join("file.txt");
/// let path = get_relative_or_absolute_path(file_path).unwrap();
/// println!("{:?}", path); // Outputs the relative or absolute path to "file.txt"
/// ```
fn get_relative_path_under_cwd(file_path: PathBuf) -> Option<PathBuf> {
    if let Ok(current_dir) = std::env::current_dir() {
        if is_child_path(current_dir, file_path.clone()) {
            truncate_cwd(&file_path)
        } else {
            Some(file_path)
        }
    } else {
        Some(file_path)
    }
}

/// Joins a relative link to a the directory of the current file and returns the canonical path
/// Unlike the std::path::PathBuf::canonicalize method, this function does not panic if the file does not exist.
///
/// This function is used to resolve the link to a target from a markdown file
/// It does not panic.
/// TODO it should return an enum and be unwrapped above
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
fn join_and_canonicalize(path: &str, current_file: PathBuf) -> PathBuf {
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

    clean_path
}

/// Takes an absolute path of a file under the current working directory
/// and returns a relative path with the current working directory removed.
/// If the input path does not start with the current working directory, or if there's an error retrieving the current working directory,
/// this function None
///
/// # Arguments
/// * `file_path` - A reference to a PathBuf object representing the absolute path from which to remove the current working directory.
///
/// # Returns
/// * A PathBuf object representing the relative path with the current working directory removed, or a copy of the input path if this is not possible.
///
/// # Examples
/// ```
/// use std::path::PathBuf;
/// let path = PathBuf::from("/home/user/documents/file.txt");
/// let truncated_path = try_truncate_cwd(&path);
/// println!("{}", truncated_path.display().unwrap());
/// // Outputs "documents/file.txt" if current working directory is "/home/user"
/// ```
fn truncate_cwd(file_path: &PathBuf) -> Option<PathBuf> {
    let current_dir = std::env::current_dir().ok()?;
    let file_path = PathBuf::from(file_path);
    Some(
        file_path
            .strip_prefix(&current_dir)
            .ok()?
            .into_iter()
            .map(|p| p.to_owned())
            .collect(),
    )
}

/// Takes two absolute paths and returns true if the second is a child of the first.
///
/// # Arguments
/// * `parent_dir` - A PathBuf object representing the parent directory.
/// * `child` - A PathBuf object representing the candidate child
///
/// # Returns
/// * A boolean value indicating whether the second path is a child of the first.
///
/// # Examples
/// ```
/// use std::path::PathBuf;
/// let parent_dir = PathBuf::from("/home/user/Notes/slipbox/");
/// let child = PathBuf::from("/home/user/Notes/slipbox/child.md");
/// assert_eq!(is_child_path(parent_dir, child), true);
/// ```
fn is_child_path(parent_dir: PathBuf, child: PathBuf) -> bool {
    if child.is_relative() {
        return false;
    }

    // Get the components of both
    let parent_components: Vec<_> = parent_dir.components().collect();
    let child_components: Vec<_> = child.components().collect();

    // If the length of child's components is less than or equal to parent's, they cannot be a child path
    if child_components.len() <= parent_components.len() {
        return false;
    }

    // Truncate the child_components
    let child_components: Vec<_> = child_components[0..parent_components.len()].to_vec();

    for (p, c) in parent_components.iter().zip(child_components.iter()) {
        if p != c {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rel_to_abspath() {
        let current_file = PathBuf::from("/home/user/Notes/slipbox/networking/dns.md");
        assert_eq!(
            join_and_canonicalize("../linux.md", current_file.clone(),),
            PathBuf::from("/home/user/Notes/slipbox/linux.md")
        );

        let current_dir = current_file.parent().unwrap().to_path_buf();

        // If passed a directory, we expect a mistaken link as the function
        // gets the directory with pop and requires the directory
        // to exist in order to test
        assert_eq!(
            join_and_canonicalize("../linux.md", current_dir),
            PathBuf::from("/home/user/Notes/linux.md")
        );

        // Also preserve relative paths
        assert_eq!(
            join_and_canonicalize("../linux.md", PathBuf::from("./networking/dns.md"),),
            PathBuf::from("linux.md")
        );
    }

    #[test]
    fn test_truncate_cwd() {
        let file_path = std::env::current_dir().unwrap().join("file.md");
        assert_eq!(truncate_cwd(&file_path), Some(PathBuf::from("file.md")));

        let file_path = std::env::current_dir().unwrap().join("foo/bar/baz/file.md");
        assert_eq!(
            truncate_cwd(&file_path),
            Some(PathBuf::from("foo/bar/baz/file.md"))
        );
    }

    #[test]
    fn test_is_child_path() {
        let current_file: PathBuf = PathBuf::from("/home/user/Notes/slipbox/");
        let file: PathBuf = PathBuf::from("/home/user/Notes/slipbox/child.md");

        // The directory of the current file is the parent directory of the file
        // So this should return true
        assert_eq!(is_child_path(current_file, file), true);

        let current_file: PathBuf = PathBuf::from("/home/user/Notes/slipbox/");
        let file: PathBuf = PathBuf::from("/home/user/Notes/not_child.md");

        // The directory of the current file is the parent directory of the file
        // So this should return true
        assert_ne!(is_child_path(current_file, file), true);
    }

    #[test]
    fn test_get_relative_path_under_cwd() {
        let current_dir = std::env::current_dir().unwrap();
        let child_file = current_dir.join("child_file");
        assert_eq!(get_relative_path_under_cwd(child_file).unwrap(), PathBuf::from("child_file"));
    }
}
