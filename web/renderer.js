// The renderer takes a slide model and owns no file, HTTP, or navigation logic.
const WIDTH = 1280;
const HEIGHT = 720;

export function createRenderer(stage, frame, canvas, onOverflow) {
  function fit() {
    const style = getComputedStyle(stage);
    const width = stage.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
    const height = stage.clientHeight - parseFloat(style.paddingTop) - parseFloat(style.paddingBottom);
    const scale = Math.max(0, Math.min(width / WIDTH, height / HEIGHT));
    frame.style.width = `${WIDTH * scale}px`;
    frame.style.height = `${HEIGHT * scale}px`;
    canvas.style.transform = `scale(${scale})`;
  }

  function measure() {
    onOverflow([...canvas.querySelectorAll('.slide-body, .slide-copy, .slide-media')].some(
      el => el.scrollHeight > el.clientHeight + 1 || el.scrollWidth > el.clientWidth + 1,
    ));
  }

  const resize = new ResizeObserver(() => { fit(); measure(); });
  resize.observe(stage);
  canvas.addEventListener('load', measure, true);
  canvas.addEventListener('error', measure, true);

  return function show(slide, theme, imageUrl) {
    renderSlide(canvas, slide, theme, imageUrl);
    frame.hidden = false;
    fit();
    measure();
  };
}

export function renderSlide(canvas, slide, theme, imageUrl) {
  canvas.style.setProperty('--slide-background', theme.background);
  canvas.style.setProperty('--slide-text', theme.text);
  canvas.style.setProperty('--slide-accent', theme.accent);

  const body = document.createElement('div');
  body.className = 'slide-body';
  // HTML is generated from Markdown by the Rust renderer, which escapes raw HTML.
  const template = document.createElement('template');
  template.innerHTML = slide.html;
  for (const image of template.content.querySelectorAll('img')) {
    image.setAttribute('src', imageUrl(image.getAttribute('src')));
  }
  body.append(template.content);
  for (const link of body.querySelectorAll('a')) {
    if (link.getAttribute('href')?.startsWith('http')) {
      link.target = '_blank';
      link.rel = 'noopener noreferrer';
    }
  }
  let layout = slide.layout;
  if (layout === 'image-right') {
    const paragraph = [...body.children].find(el =>
      el.tagName === 'P' && el.childElementCount === 1 &&
      el.firstElementChild.tagName === 'IMG' && !el.textContent.trim());
    if (paragraph) {
      const media = document.createElement('div');
      media.className = 'slide-media';
      media.append(paragraph.firstElementChild);
      paragraph.remove();
      const copy = document.createElement('div');
      copy.className = 'slide-copy';
      copy.append(...body.childNodes);
      body.append(copy, media);
    } else {
      layout = 'content';
    }
  }
  canvas.dataset.layout = layout;
  canvas.replaceChildren(body);
}
