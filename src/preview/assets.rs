use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use std::path::{Component, Path, PathBuf};

const PATH_ENCODING: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'/')
    .remove(b'.')
    .remove(b'-')
    .remove(b'_')
    .remove(b'~');

pub(super) fn image_path(input: &str) -> Option<PathBuf> {
    if input.contains(['?', '#']) {
        return None;
    }
    let input = percent_decode_str(input).decode_utf8().ok()?;
    if input.contains([':', '\\']) || input.chars().any(char::is_control) {
        return None;
    }
    let mut relative = PathBuf::new();
    for component in Path::new(input.as_ref()).components() {
        match component {
            Component::CurDir => (),
            Component::Normal(part) => relative.push(part),
            _ => return None,
        }
    }
    image_mime(&relative)?;
    Some(relative)
}

pub(super) fn asset_url(path: &Path) -> Option<String> {
    let encoded = utf8_percent_encode(path.to_str()?, PATH_ENCODING);
    Some(format!("/assets/{encoded}"))
}

pub(super) fn image_mime(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "avif" => Some("image/avif"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_url_keeps_unreserved_characters_literal() {
        let path = image_path("./images/slide_1%20photo.png").unwrap();
        assert_eq!(
            asset_url(&path).as_deref(),
            Some("/assets/images/slide_1%20photo.png")
        );
    }

    #[test]
    fn image_paths_are_local_raster_files() {
        assert_eq!(
            image_path("./images/a%20b.PNG"),
            Some("images/a b.PNG".into())
        );
        for bad in [
            "../a.png",
            "images/%2e%2e/a.png",
            "/a.png",
            "//host/a.png",
            "http://host/a.png",
            "images/a%00.png",
            "a.svg",
            "a.png?x",
            "a\\b.png",
        ] {
            assert_eq!(image_path(bad), None, "{bad}");
        }
    }
}
