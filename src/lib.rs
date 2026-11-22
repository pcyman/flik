//! Markdown-to-slide rendering, independent of the preview transport.
mod slide;
mod theme;

use pulldown_cmark::{Event, Options, Parser};
use serde::{Deserialize, Serialize};
pub use slide::Slide;
pub use theme::Theme;
use theme::ThemeSpec;

#[derive(Debug, Serialize)]
pub struct Deck {
    pub title: String,
    pub theme: Theme,
    pub slides: Vec<Slide>,
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct Metadata {
    title: Option<String>,
    theme: ThemeSpec,
}

/// Parse a deck. The caller decides whether and how an image URL can be loaded.
pub fn render(
    source: &str,
    fallback_title: &str,
    mut image_url: impl FnMut(&str) -> Option<String>,
) -> Result<Deck, String> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let (metadata, body) = front_matter(source)?;
    let theme = Theme::resolve(metadata.theme)?;
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let mut depth = 0;
    let mut start = 0;
    let mut slides = Vec::new();
    let mut add = |raw: &str| -> Result<(), String> {
        if !raw.trim().is_empty() {
            let number = slides.len() + 1;
            slides.push(
                slide::render(raw, options, &mut image_url)
                    .map_err(|err| format!("Slide {number}: {err}"))?,
            );
        }
        Ok(())
    };
    for (event, range) in Parser::new_ext(body, options).into_offset_iter() {
        match event {
            Event::Start(_) => depth += 1,
            Event::End(_) => depth -= 1,
            Event::Rule if depth == 0 && body[range.clone()].trim() == "---" => {
                add(&body[start..range.start])?;
                start = range.end;
            }
            _ => {}
        }
    }
    add(&body[start..])?;
    if slides.is_empty() {
        return Err("The deck has no slides".into());
    }
    Ok(Deck {
        title: metadata.title.unwrap_or_else(|| fallback_title.into()),
        theme,
        slides,
    })
}

fn front_matter(source: &str) -> Result<(Metadata, &str), String> {
    let mut lines = source.split_inclusive('\n');
    let first = lines.next().unwrap_or_default();
    if first.trim_end_matches(['\r', '\n']) != "---" {
        return Ok((Metadata::default(), source));
    }
    let mut end = first.len();
    for line in lines {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            let yaml = &source[first.len()..end];
            let metadata = if yaml.trim().is_empty() {
                Metadata::default()
            } else {
                serde_yaml::from_str(yaml).map_err(|err| format!("Invalid front matter: {err}"))?
            };
            return Ok((metadata, &source[end + line.len()..]));
        }
        end += line.len();
    }
    Err("Unclosed YAML front matter (expected a closing ---)".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::to_value;

    #[test]
    fn parses_theme_layouts_and_commonmark_boundaries() {
        let source = "---\r\ntitle: Demo\r\ntheme:\r\n  base: midnight\r\n  accent: '#abc'\r\n---\r\n\r\n<!-- layout: title -->\n\n# First\n\n```md\n---\n```\n\n> ---\n\nSetext\n---\n\n---\n\n<!-- layout: image-right -->\n\n![Diagram](pic.png)\n";
        let deck = render(source, "fallback", |p| Some(format!("/assets/{p}"))).unwrap();
        assert_eq!(deck.title, "Demo");
        let theme = to_value(deck.theme).unwrap();
        assert_eq!(theme["accent"], "#abc");
        assert_eq!(theme["background"], "#101827");
        assert_eq!(deck.slides.len(), 2);
        let first = to_value(&deck.slides[0]).unwrap();
        assert_eq!(first["layout"], "title");
        assert!(first["html"].as_str().unwrap().contains("<h2>Setext</h2>"));
        let second = to_value(&deck.slides[1]).unwrap();
        assert_eq!(second["layout"], "image-right");
        assert!(
            second["html"]
                .as_str()
                .unwrap()
                .contains("src=\"/assets/pic.png\" alt=\"Diagram\"")
        );
    }

    #[test]
    fn escapes_raw_html_and_filters_urls() {
        let deck = render(
            "<div>raw</div>\n\n[x](ftp://example.test) ![alt](https://example.test/p.png)",
            "fallback",
            |_| None,
        )
        .unwrap();
        let slide = to_value(&deck.slides[0]).unwrap();
        let html = slide["html"].as_str().unwrap();
        assert!(html.contains("&lt;div&gt;"));
        assert!(html.contains("href=\"#\""));
        assert!(html.contains("src=\"data:,\" alt=\"alt\""));
    }

    #[test]
    fn detects_layout_after_whitespace_only_lines() {
        let deck = render("  \n<!-- layout: title -->\n# First\n\n---\n\n \t\n<!-- layout: image-right -->\n# Second", "fallback", |_| None).unwrap();
        let first = to_value(&deck.slides[0]).unwrap();
        assert_eq!(first["layout"], "title");
        let second = to_value(&deck.slides[1]).unwrap();
        assert_eq!(second["layout"], "image-right");
    }

    #[test]
    fn reports_authoring_errors() {
        for (source, error) in [
            ("---\ntitle: Oops", "Unclosed"),
            ("---\ntheme: other\n---\n# Title", "Unknown theme"),
            ("<!-- layout: other -->\n# Title", "Slide 1"),
            ("\n\n", "no slides"),
        ] {
            assert!(
                render(source, "fallback", |_| None)
                    .unwrap_err()
                    .contains(error)
            );
        }
    }
}
