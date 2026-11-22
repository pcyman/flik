//! Per-slide layout parsing and Markdown-to-HTML rendering.
use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd, html};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Slide {
    layout: Layout,
    html: String,
    // The default serialized slide is safe for audience views and export.
    #[serde(skip)]
    pub notes: String,
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
    let mut notes = Vec::new();
    let mut depth = 0;
    let mut skip_html_block = false;
    let mut unclosed_notes = false;
    let events = Parser::new_ext(body, options)
        .into_offset_iter()
        .filter_map(|(event, range)| {
            if skip_html_block {
                if matches!(event, Event::End(TagEnd::HtmlBlock)) {
                    skip_html_block = false;
                }
                return None;
            }
            match &event {
                Event::Start(Tag::HtmlBlock) if depth == 0 => {
                    let mut remainder = &body[range];
                    while let Some(comment) = remainder.trim_start().strip_prefix("<!-- notes:") {
                        skip_html_block = true;
                        let Some((note, tail)) = comment.split_once("-->") else {
                            unclosed_notes = true;
                            return None;
                        };
                        if !note.trim().is_empty() {
                            notes.push(note.trim().replace("\r\n", "\n"));
                        }
                        remainder = tail;
                    }
                    if skip_html_block {
                        return (!remainder.trim().is_empty())
                            .then(|| Event::Text(remainder.into()));
                    }
                    depth += 1;
                }
                Event::Start(_) => depth += 1,
                Event::End(_) => depth -= 1,
                _ => {}
            }
            Some(event)
        });
    let events = events.map(|event| match event {
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
    if unclosed_notes {
        return Err("Unclosed notes comment (expected -->)".into());
    }
    Ok(Slide {
        layout,
        html: rendered,
        notes: notes.join("\n\n"),
    })
}
