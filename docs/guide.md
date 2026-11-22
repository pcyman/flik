# Flik user guide

## Preview and present

Run `cargo run --locked -- path/to/deck.md`. Flik prints a local preview URL and opens it with `xdg-open` by default.

- `--app` opens a standalone Chromium window and requires `chromium` on your `PATH`.
- `--presenter` opens synced Audience and Presenter views.
- `--no-open` prints the URLs without launching a browser. It cannot be combined with `--app`.
- `--help` lists CLI options. When using `cargo run`, place Flik’s flags after the standalone `--`.

Press **F** or click **Present** for full screen, and **Esc** to exit. Use the **arrow keys** or **Page Up/Page Down** to navigate, **Space** to advance, and **Home/End** to jump to the first or last slide. Press **Ctrl+C** in the terminal to stop the preview.

Saving Markdown or local images refreshes the preview and keeps your slide position where possible. Invalid edits leave the last valid deck visible. Overflowing slide content is clipped, with a warning in preview.

## Presenter mode

Run `cargo run --locked -- --presenter examples/demo.md`. Move the **Audience** view to the projector or share that window, then press **F** there for full screen. Keep the **Presenter** view on your own screen. Both views can navigate and stay in sync through the same running Flik session, even in different browser profiles. Your default browser may open tabs instead of windows; move one tab to a new window, use `--app` for standalone Chromium windows, or reopen either URL printed in the terminal.

The presenter view shows the current slide, its speaker notes, and the next slide. The elapsed timer starts only when you click **Start**. **Pause** and **Resume** let you take a break; **Reset** stops the timer and returns it to zero. Reloading the presenter page also resets the timer.

Add notes with a top-level `notes` code fence anywhere in the slide body, after the layout comment if there is one:

````markdown
## Demo

```notes
Ask who has tried the old setup.
Pause before showing the demo.
```
````

Notes are plain text and are omitted from the audience view and PDF export. Share just the audience window, not the whole desktop or the Markdown source if the notes are sensitive.

## Export to PDF

Click **Export PDF** in the preview toolbar, then choose **Save as PDF** in the browser’s print dialog. Flik prints the whole deck in slide order, with one 16:9 slide per page and no preview controls. If the browser adds headers or footers or rescales the slides, turn off headers and footers and use the default scale and page size.

## Write a deck

The [example deck](../examples/demo.md) shows all three layouts and a custom theme.

- **Slides:** Put `---` on its own line, with blank lines around it. A `---` on the first line instead opens optional YAML front matter, which can set the deck’s `title` and `theme`.
- **Themes:** Use `theme: light` (the default), `theme: midnight`, or a [custom theme](#custom-themes).
- **Layouts:** Use a comment such as `<!-- layout: title -->` at the start of a slide. `content` is the default, `title` centers the content, and `image-right` moves the first image on its own paragraph to the right. Without such an image, it falls back to `content`.
- **Images:** Use standard Markdown, e.g. `![Chart](./images/chart.png)`. Paths are relative to the deck and must stay within its directory. PNG, JPEG, GIF, WebP, and AVIF are supported. Remote images, SVG, and raw HTML are not supported.

### Custom themes

Put the theme in the YAML front matter at the top of your Markdown file. It applies to the whole deck. This example keeps the `midnight` background and text colors but changes the accent:

```markdown
---
title: Design review
theme:
  base: midnight
  accent: "#f4b860"
---

<!-- layout: title -->

# Design review
```

Override `background`, `text`, or `accent` with quoted `#RGB` or `#RRGGBB` colors. Omitted colors come from the `base` preset. If you omit `base`, Flik uses `light`.
