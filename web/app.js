import { createRenderer } from './renderer.js';

const el = id => document.getElementById(id);
const stage = el('stage');
const previous = el('previous');
const next = el('next');
const present = el('present');
const error = el('error');
let deck;
let imageRevisions;
let index = 0;
let lastResponse;
let loadError = '';
let fullscreenError = '';

const show = createRenderer(stage, el('frame'), el('slide'), overflowing => {
  el('warning').hidden = !overflowing;
});

function draw() {
  el('placeholder').hidden = true;
  el('deck-title').textContent = deck.title;
  document.title = `${deck.title} · Flik`;
  el('position').value = `${index + 1} / ${deck.slides.length}`;
  previous.disabled = index === 0;
  next.disabled = index === deck.slides.length - 1;
  present.disabled = !document.fullscreenEnabled;
  show(deck.slides[index], deck.theme, src => {
    if (!Object.hasOwn(imageRevisions, src)) return src;
    return `${src}?v=${imageRevisions[src] ?? 'missing'}`;
  });
}

function goTo(target) {
  if (!deck) return;
  const clamped = Math.max(0, Math.min(target, deck.slides.length - 1));
  if (clamped === index) return;
  index = clamped;
  draw();
}

function displayError() {
  const message = loadError || fullscreenError;
  if (error.textContent !== message) error.textContent = message;
  error.hidden = !message;
  if (!deck) el('placeholder').textContent = 'Waiting for a valid deck…';
}

async function refresh() {
  try {
    const response = await fetch('/api/deck', { signal: AbortSignal.timeout(5000) });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const text = await response.text();
    if (text !== lastResponse) {
      const update = JSON.parse(text);
      if (update.deck) {
        deck = update.deck;
        imageRevisions = update.imageRevisions;
        index = Math.min(index, deck.slides.length - 1);
        draw();
      }
      lastResponse = text;
      loadError = update.error || '';
    }
  } catch {
    lastResponse = undefined;
    loadError = 'Preview disconnected. Retrying…';
  } finally {
    displayError();
    setTimeout(refresh, 750);
  }
}

async function toggleFullscreen() {
  if (!deck || !document.fullscreenEnabled) return;
  try {
    if (document.fullscreenElement) await document.exitFullscreen();
    else await stage.requestFullscreen();
    fullscreenError = '';
  } catch {
    fullscreenError = 'The browser could not enter or exit full screen.';
  }
  displayError();
}

previous.addEventListener('click', () => goTo(index - 1));
next.addEventListener('click', () => goTo(index + 1));
present.addEventListener('click', toggleFullscreen);
window.addEventListener('keydown', event => {
  if (event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey || !deck) return;
  if (event.key === ' ' && event.target instanceof Element && event.target.closest('button, a')) return;
  switch (event.key) {
    case 'ArrowLeft': case 'PageUp': goTo(index - 1); break;
    case 'ArrowRight': case 'PageDown': case ' ': goTo(index + 1); break;
    case 'Home': goTo(0); break;
    case 'End': goTo(deck.slides.length - 1); break;
    case 'f': case 'F': if (!event.repeat) toggleFullscreen(); break;
    default: return;
  }
  event.preventDefault();
});

refresh();
