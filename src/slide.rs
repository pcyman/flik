//! Per-slide layout parsing and Markdown-to-HTML rendering.
use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, html};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Slide {
    layout: Layout,
    html: String,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
enum Layout {
    #[default]
    Content,
    Title,
    ImageRight,
}

pub(super) fn render(
    raw: &str,
    options: Options,
    image_url: &mut impl FnMut(&str) -> Option<String>,
) -> Result<Slide, String> {
    let leading_blank_bytes = raw
        .split_inclusive('\n')
        .take_while(|line| line.trim().is_empty())
        .map(str::len)
        .sum::<usize>();
    let trimmed = &raw[leading_blank_bytes..];
    let (first, rest) = trimmed.split_once('\n').unwrap_or((trimmed, ""));
    let (layout, body) = if let Some(marker) = first.trim().strip_prefix("<!-- layout:") {
        let name = marker
            .strip_suffix("-->")
            .ok_or("Unclosed layout comment")?
            .trim();
        let layout = match name {
            "title" => Layout::Title,
            "content" => Layout::Content,
            "image-right" => Layout::ImageRight,
            _ => return Err(format!("Unknown layout {name:?}")),
        };
        (layout, rest)
    } else {
        (Layout::Content, raw)
    };
    let events = Parser::new_ext(body, options).map(|event| match event {
        Event::Html(text) | Event::InlineHtml(text) => Event::Text(text),
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let Some(url) = image_url(&dest_url) else {
                return Event::Start(Tag::Image {
                    link_type,
                    dest_url: CowStr::Borrowed("data:,"),
                    title,
                    id,
                });
            };
            Event::Start(Tag::Image {
                link_type,
                dest_url: url.into(),
                title,
                id,
            })
        }
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let allowed = ["https://", "http://", "mailto:", "#"]
                .iter()
                .any(|prefix| dest_url.starts_with(prefix));
            Event::Start(Tag::Link {
                link_type,
                dest_url: if allowed { dest_url } else { "#".into() },
                title,
                id,
            })
        }
        other => other,
    });
    let mut rendered = String::new();
    html::push_html(&mut rendered, events);
    Ok(Slide {
        layout,
        html: rendered,
    })
}
