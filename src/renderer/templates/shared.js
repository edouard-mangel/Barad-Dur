
(function() {
  'use strict';

  var R = window.R;

  // Palette for ownership colours
  var PALETTE = [
    '#3b82f6','#10b981','#f59e0b','#ef4444','#8b5cf6',
    '#06b6d4','#ec4899','#84cc16','#f97316','#6366f1'
  ];

  /* ---- helpers ---- */
  function txt(s) { return document.createTextNode(String(s)); }

  /* One el() attribute: style objects, className, onClick listeners, and
     plain attributes each apply differently. */
  function applyAttr(node, k, v) {
    if (k === 'style') {
      for (var s in v) { node.style[s] = v[s]; }
    } else if (k === 'className') {
      node.className = v;
    } else if (k === 'onClick') {
      node.addEventListener('click', v);
    } else {
      node.setAttribute(k, v);
    }
  }

  function el(tag, attrs) {
    var node = document.createElement(tag);
    if (attrs) {
      for (var k in attrs) { applyAttr(node, k, attrs[k]); }
    }
    for (var i = 2; i < arguments.length; i++) {
      var child = arguments[i];
      if (child == null) continue;
      if (typeof child === 'string' || typeof child === 'number') {
        node.append(txt(child));
      } else {
        node.append(child);
      }
    }
    return node;
  }

  function svgEl(tag, attrs) {
    var node = document.createElementNS('http://www.w3.org/2000/svg', tag);
    if (attrs) {
      for (var k in attrs) {
        node.setAttribute(k, attrs[k]);
      }
    }
    for (var i = 2; i < arguments.length; i++) {
      var child = arguments[i];
      if (child == null) continue;
      node.append(child);
    }
    return node;
  }

  /* Band thresholds come from the report itself (scorer/types.rs is the
     single source of truth); the fallback only covers pre-threshold reports. */
  var BANDS = (R && R.score_thresholds) || { good_min: 71, warn_min: 41 };

  function scoreColor(s) {
    return s >= BANDS.good_min ? 'var(--c-good)'
      : s >= BANDS.warn_min ? 'var(--c-warn)'
      : 'var(--c-danger)';
  }

  function fileParts(path) {
    var idx = path.lastIndexOf('/');
    if (idx === -1) return { dir: '', name: path };
    return { dir: path.slice(0, idx + 1), name: path.slice(idx + 1) };
  }

  function scoreBar(score) {
    var wrap = el('div', { className: 'bar-wrap' });
    var fill = el('div', { className: 'bar-fill', style: {
      width: score + '%',
      background: scoreColor(score)
    }});
    wrap.append(fill);
    return wrap;
  }

  function inlineBar(pct, color) {
    var track = el('div', { className: 'track' });
    var fill = el('div', { className: 'fill', style: {
      width: Math.min(100, pct) + '%',
      background: color || '#3b82f6'
    }});
    track.append(fill);
    return track;
  }

  function chip(text, bg, fg) {
    var c = el('span', { className: 'chip', style: {
      background: bg || '#1e293b',
      color: fg || '#e2e8f0'
    }});
    c.append(txt(text));
    return c;
  }

  function fmt(v, n) {
    if (v == null) return '—';
    var x = +v;
    return Number.isFinite(x) ? x.toFixed(n == null ? 0 : n) : '—';
  }
