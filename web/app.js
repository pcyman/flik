import { createRenderer, renderSlide } from './renderer.js';

const el = id => document.getElementById(id);
const stage = el('stage');
const previous = el('previous');
const next = el('next');
const present = el('present');
const exportPdf = el('export-pdf');
const printDeck = el('print-deck');
const error = el('error');
const presenter = location.pathname === '/presenter';
const synced = presenter || location.pathname === '/audience';
const viewLabel = presenter ? 'Presenter' : synced ? 'Audience' : '';
let notes = [];
let deck;
let imageRevisions;
let index = 0;
let lastResponse;
let loadError = '';
let fullscreenError = '';
let exportError = '';
let syncError = '';
let sharedIndex = 0;
let pendingIndex;

const show = createRenderer(stage, el('frame'), el('slide'), overflowing => {
  el('warning').hidden = !overflowing;
});

const notesText = el('notes-text');
let notesIndex;
let showUpcoming;
if (viewLabel) {
  el('deck-title').textContent = viewLabel;
  document.title = `${viewLabel} · Flik`;
}
if (presenter) {
  document.body.classList.add('presenter');
  el('preview-actions').hidden = true;
  for (const id of ['talk-timer', 'speaker-notes', 'upcoming']) el(id).hidden = false;
  showUpcoming = createRenderer(el('next-stage'), el('next-frame'), el('next-slide'), () => {});
  setupTimer();
}

function setupTimer() {
  const toggle = el('timer-toggle');
  const reset = el('timer-reset');
  let startedAt = null;
  let accumulated = 0;
  let ticker;

  function update() {
    const elapsed = accumulated + (startedAt === null ? 0 : performance.now() - startedAt);
    const seconds = Math.floor(elapsed / 1000);
    el('elapsed').textContent = [Math.floor(seconds / 3600), Math.floor(seconds / 60) % 60, seconds % 60]
      .map(value => String(value).padStart(2, '0')).join(':');
  }

  toggle.addEventListener('click', () => {
    const now = performance.now();
    if (startedAt === null) {
      startedAt = now;
      ticker = setInterval(update, 200);
      toggle.textContent = 'Pause';
      reset.disabled = false;
    } else {
      accumulated += now - startedAt;
      startedAt = null;
      clearInterval(ticker);
      toggle.textContent = 'Resume';
    }
    update();
  });
  reset.addEventListener('click', () => {
    startedAt = null;
    accumulated = 0;
    clearInterval(ticker);
    toggle.textContent = 'Start';
    reset.disabled = true;
    update();
  });
}

// Serialize reads and writes so an older poll cannot undo a newer local move.
async function syncPosition() {
  const target = pendingIndex;
  pendingIndex = undefined;
  let retry = true;
  try {
    const response = await fetch('/api/position', {
      method: target === undefined ? 'GET' : 'POST',
      headers: target === undefined ? undefined : { 'X-Flik-Slide': String(target) },
      signal: AbortSignal.timeout(3000),
    });
    if (!response.ok) {
      retry = response.status >= 500;
      throw new Error(`HTTP ${response.status}`);
    }
    const position = await response.json();
    if (pendingIndex === undefined) {
      sharedIndex = position;
      goTo(position, false);
    }
    syncError = '';
  } catch {
    if (retry && pendingIndex === undefined) pendingIndex = target;
    syncError = 'Slide sync disconnected. Retrying…';
  } finally {
    displayError();
    setTimeout(syncPosition, syncError || pendingIndex === undefined ? 150 : 0);
  }
}

function imageUrl(src) {
  if (!Object.hasOwn(imageRevisions, src)) return src;
  return `${src}?v=${imageRevisions[src] ?? 'missing'}`;
}

function draw() {
  el('placeholder').hidden = true;
  const title = viewLabel ? `${viewLabel} · ${deck.title}` : deck.title;
  el('deck-title').textContent = title;
  document.title = `${title} · Flik`;
  el('position').value = `${index + 1} / ${deck.slides.length}`;
  previous.disabled = index === 0;
  next.disabled = index === deck.slides.length - 1;
  present.disabled = !document.fullscreenEnabled;
  show(deck.slides[index], deck.theme, imageUrl);
  if (!presenter) return;

  const text = notes[index] || 'No speaker notes for this slide.';
  if (notesText.textContent !== text) notesText.textContent = text;
  if (notesIndex !== index) notesText.scrollTop = 0;
  notesIndex = index;
  const upcoming = deck.slides[index + 1];
  el('next-placeholder').hidden = Boolean(upcoming);
  if (upcoming) showUpcoming(upcoming, deck.theme, imageUrl);
  else {
    el('next-frame').hidden = true;
    el('next-placeholder').textContent = 'End of deck';
  }
}

function goTo(target, share = synced) {
  if (!deck) return;
  const clamped = Math.max(0, Math.min(target, deck.slides.length - 1));
  if (clamped === index) return;
  index = clamped;
  if (share) sharedIndex = pendingIndex = index;
  draw();
}

function displayError() {
  const message = exportError || loadError || fullscreenError || syncError;
  if (error.textContent !== message) error.textContent = message;
  error.hidden = !message;
  if (!deck && loadError) el('placeholder').textContent = 'Waiting for a valid deck…';
}

async function refresh() {
  try {
    const response = await fetch(presenter ? '/api/presenter' : '/api/deck', { signal: AbortSignal.timeout(5000) });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const text = await response.text();
    if (text !== lastResponse) {
      const update = JSON.parse(text);
      if (update.deck) {
        if (!deck) exportPdf.disabled = false;
        deck = update.deck;
        imageRevisions = update.imageRevisions;
        notes = update.notes ?? [];
        index = Math.min(synced ? sharedIndex : index, deck.slides.length - 1);
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

function finishExport() {
  document.body.classList.remove('printing-deck');
  printDeck.replaceChildren();
  exportPdf.disabled = !deck;
  exportPdf.textContent = 'Export PDF';
}

async function printPresentation() {
  if (!deck || exportPdf.disabled) return;
  exportPdf.disabled = true;
  exportPdf.textContent = 'Preparing PDF…';
  exportError = '';
  displayError();

  try {
    // Render the whole snapshot before yielding to the live refresh loop.
    const { slides, theme } = deck;
    printDeck.replaceChildren(...slides.map(slide => {
      const canvas = document.createElement('article');
      canvas.className = 'slide';
      renderSlide(canvas, slide, theme, imageUrl);
      return canvas;
    }));

    // Give off-screen images time to load, without letting a stalled request block printing.
    let timeout;
    try {
      await Promise.race([
        Promise.allSettled(Array.from(printDeck.querySelectorAll('img'), image => image.decode())),
        new Promise(resolve => { timeout = setTimeout(resolve, 5000); }),
      ]);
    } finally {
      clearTimeout(timeout);
    }

    document.body.classList.add('printing-deck');
    let printStarted = false;
    const markStarted = () => { printStarted = true; };
    window.addEventListener('beforeprint', markStarted, { once: true });
    try {
      window.print();
    } finally {
      window.removeEventListener('beforeprint', markStarted);
    }
    if (!printStarted) throw new Error('Printing was blocked');
  } catch {
    finishExport();
    exportError = 'The browser could not open the print dialog.';
    displayError();
  }
}

exportPdf.addEventListener('click', printPresentation);
window.addEventListener('afterprint', () => {
  if (document.body.classList.contains('printing-deck')) finishExport();
});
previous.addEventListener('click', () => goTo(index - 1));
next.addEventListener('click', () => goTo(index + 1));
present.addEventListener('click', toggleFullscreen);
window.addEventListener('keydown', event => {
  if (event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey || !deck) return;
  if (presenter && event.target === notesText) return;
  if (event.key === ' ' && event.target instanceof Element && event.target.closest('button, a')) return;
  switch (event.key) {
    case 'ArrowLeft': case 'PageUp': goTo(index - 1); break;
    case 'ArrowRight': case 'PageDown': case ' ': goTo(index + 1); break;
    case 'Home': goTo(0); break;
    case 'End': goTo(deck.slides.length - 1); break;
    case 'f': case 'F':
      if (presenter) return;
      if (!event.repeat) toggleFullscreen();
      break;
    default: return;
  }
  event.preventDefault();
});

refresh();
if (synced) syncPosition();
