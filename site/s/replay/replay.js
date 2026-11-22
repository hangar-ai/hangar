/* ═══════════════════════════════════════════════════════════════
   A HangarAI replay (index.html, replay.css)

   Plays back a workspace's orchestration: the board's cards moving
   from To do to Done as the agents took them, sent deliveries back
   and landed them, with a timeline to scrub. The data is the page's
   own (<script id="replay-data">, hangar-core's replay::Replay,
   version 1): nothing is fetched, nothing is stored, nothing measures
   the visitor. Every text of the data goes in as text, never as HTML.

   The first half is plain functions (the data checked, the board at a
   moment, the time compressed), which check.mjs runs under Node; the
   second draws them.
   ═══════════════════════════════════════════════════════════════ */
(function () {
  'use strict';

  var SCHEMA = 1;
  var COLUMNS = ['todo', 'doing', 'review', 'done'];
  var KINDS = ['created', 'started', 'progress', 'failed', 'review', 'landed', 'done'];
  var WHYS = ['check', 'conflict', 'changes', 'other'];
  var ROLES = ['orchestrator', 'worker'];
  var HUES = 8;
  var CODE = /^[a-z0-9](?:[a-z0-9]|-(?!-)){1,18}[a-z0-9]$/;
  var HIDDEN = /\[(secret|path|email|name|link)\]/g;
  var SHOT = /^shots\/\d{1,3}\.(png|jpe?g|webp|gif)$/;

  var COLUMN_OF = {
    created: 'todo', started: 'doing', failed: 'doing', review: 'review', landed: 'done', done: 'done'
  };

  /* ── the data, checked ─────────────────────────────────────── */

  function isInt(n) { return typeof n === 'number' && isFinite(n) && Math.floor(n) === n && n >= 0; }
  function isStr(s) { return typeof s === 'string'; }
  function optStr(s) { return s === undefined || isStr(s); }
  function optInt(n) { return n === undefined || isInt(n); }

  // Why `data` is not a replay this page plays, or null.
  function invalid(data) {
    if (!data || typeof data !== 'object') return 'not an object';
    if (data.v !== SCHEMA) return 'version ' + data.v + ', not ' + SCHEMA;
    if (!isStr(data.title)) return 'title';
    if (!isInt(data.from) || !isInt(data.to) || !isInt(data.made) || data.to < data.from) return 'times';
    var t = data.totals;
    if (!t || typeof t !== 'object') return 'totals';
    var figures = ['tasks', 'landed', 'done', 'failed', 'agents', 'duration', 'added', 'removed', 'files'];
    for (var f = 0; f < figures.length; f++) if (!isInt(t[figures[f]])) return 'totals.' + figures[f];
    if (!Array.isArray(data.agents) || !Array.isArray(data.tasks) || !Array.isArray(data.events)) return 'lists';
    var names = {};
    for (var a = 0; a < data.agents.length; a++) {
      var agent = data.agents[a];
      if (!agent || !isStr(agent.name) || ROLES.indexOf(agent.role) === -1 || !isInt(agent.tasks) || !isInt(agent.landed)) {
        return 'agents[' + a + ']';
      }
      names[agent.name] = true;
    }
    var ids = {};
    for (var c = 0; c < data.tasks.length; c++) {
      var card = data.tasks[c];
      if (!card || !isStr(card.id) || !isStr(card.title) || ids[card.id]) return 'tasks[' + c + ']';
      if (card.labels !== undefined && (!Array.isArray(card.labels) || !card.labels.every(isStr))) return 'tasks[' + c + '].labels';
      if (card.start !== undefined && COLUMNS.indexOf(card.start) === -1) return 'tasks[' + c + '].start';
      if (card.agent !== undefined && !(isStr(card.agent) && names[card.agent])) return 'tasks[' + c + '].agent';
      ids[card.id] = true;
    }
    var last = 0;
    for (var e = 0; e < data.events.length; e++) {
      var ev = data.events[e];
      var at = 'events[' + e + ']';
      if (!ev || !isInt(ev.at) || ev.at < last || !ids[ev.task] || KINDS.indexOf(ev.type) === -1) return at;
      if (ev.agent !== undefined && !(isStr(ev.agent) && names[ev.agent])) return at + '.agent';
      if (ev.percent !== undefined && !(isInt(ev.percent) && ev.percent <= 100)) return at + '.percent';
      if (!optStr(ev.note) || !optStr(ev.summary) || !optStr(ev.commit)) return at + '.text';
      if (ev.why !== undefined && WHYS.indexOf(ev.why) === -1) return at + '.why';
      if (!optInt(ev.added) || !optInt(ev.removed) || !optInt(ev.files)) return at + '.changes';
      if (ev.checked !== undefined && typeof ev.checked !== 'boolean') return at + '.checked';
      if (ev.reviewed !== undefined && typeof ev.reviewed !== 'boolean') return at + '.reviewed';
      if (ev.pictures !== undefined && !(Array.isArray(ev.pictures) && ev.pictures.every(function (p) { return isStr(p) && SHOT.test(p); }))) {
        return at + '.pictures';
      }
      last = ev.at;
    }
    if (data.referral !== undefined && !(isStr(data.referral) && CODE.test(data.referral))) return 'referral';
    return null;
  }

  /* ── the board at a moment ─────────────────────────────────── */

  // Where each event sits on the timeline, 0 to 1. Real gaps are kept
  // in proportion up to three times the usual one, so a night's pause
  // takes a moment, not the whole bar; every event gets a little room.
  function positions(events) {
    var n = events.length;
    if (!n) return [];
    var gaps = [];
    for (var i = 1; i < n; i++) gaps.push(Math.max(0, events[i].at - events[i - 1].at));
    var sorted = gaps.slice().sort(function (a, b) { return a - b; });
    var usual = sorted.length ? sorted[Math.floor(sorted.length / 2)] : 0;
    var cap = Math.max(usual * 3, 1);
    var room = cap * 0.35;
    var x = [0];
    for (var k = 1; k < n; k++) x.push(x[k - 1] + room + Math.min(gaps[k - 1], cap));
    var span = x[n - 1] || 1;
    // A breath before the first event and after the last.
    var lead = 0.03;
    return x.map(function (v) { return n === 1 ? 0.5 : lead + (v / span) * (1 - 2 * lead); });
  }

  function model(data) {
    var agentIndex = {};
    data.agents.forEach(function (agent, i) { agentIndex[agent.name] = i % HUES; });
    var cards = {};
    data.tasks.forEach(function (card, i) { cards[card.id] = { card: card, order: i }; });
    return {
      data: data,
      cards: cards,
      agentIndex: agentIndex,
      events: data.events,
      at: positions(data.events)
    };
  }

  // How many events have happened at `s` (0 to 1).
  function reached(m, s) {
    var n = 0;
    while (n < m.at.length && m.at[n] <= s + 1e-9) n++;
    return n;
  }

  // The board after the first `count` events.
  function stateAt(m, count) {
    var tasks = {};
    m.data.tasks.forEach(function (card, i) {
      tasks[card.id] = {
        id: card.id,
        column: card.start || null,
        agent: card.start ? (card.agent || null) : null,
        percent: null,
        back: 0,
        landed: null,
        since: -1 - (m.data.tasks.length - i)
      };
    });
    var counts = { landed: 0, failed: 0, added: 0, removed: 0, files: 0 };
    var active = {};
    for (var i = 0; i < count; i++) {
      var ev = m.events[i];
      var task = tasks[ev.task];
      var column = COLUMN_OF[ev.type];
      if (ev.agent) task.agent = ev.agent;
      if (column && column !== task.column) {
        task.column = column;
        task.since = i;
      }
      if (ev.type === 'started') task.percent = null;
      if (ev.type === 'progress' && ev.percent !== undefined) task.percent = ev.percent;
      if (ev.type === 'failed') { task.back += 1; counts.failed += 1; }
      if (ev.type === 'landed') {
        task.landed = ev;
        counts.landed += 1;
        counts.added += ev.added || 0;
        counts.removed += ev.removed || 0;
        counts.files += ev.files || 0;
      }
      if (ev.agent) active[ev.agent] = true;
    }
    var columns = { todo: [], doing: [], review: [], done: [] };
    Object.keys(tasks).forEach(function (id) {
      var task = tasks[id];
      if (task.column) columns[task.column].push(task);
    });
    // The latest arrival first: what just moved is in sight.
    COLUMNS.forEach(function (name) {
      columns[name].sort(function (a, b) { return b.since - a.since; });
    });
    var working = {};
    columns.doing.forEach(function (task) { if (task.agent) working[task.agent] = true; });
    var shown = 0;
    COLUMNS.forEach(function (name) { shown += columns[name].length; });
    return {
      tasks: tasks,
      columns: columns,
      counts: counts,
      shown: shown,
      done: columns.done.length,
      active: active,
      working: working
    };
  }

  // The real time at `s`, between the events around it.
  function timeAt(m, s) {
    var ev = m.events;
    if (!ev.length) return m.data.from;
    if (s <= m.at[0]) return ev[0].at;
    for (var i = 1; i < ev.length; i++) {
      if (s < m.at[i]) {
        var part = (s - m.at[i - 1]) / ((m.at[i] - m.at[i - 1]) || 1);
        return Math.round(ev[i - 1].at + part * (ev[i].at - ev[i - 1].at));
      }
    }
    return ev[ev.length - 1].at;
  }

  /* ── words ─────────────────────────────────────────────────── */

  var WORDS = {
    en: {
      eyebrow: 'HangarAI replay',
      board: 'The board',
      happened: 'What happened',
      agents: 'The agents',
      made: 'Made with HangarAI',
      try: 'Try it',
      columns: { todo: 'To do', doing: 'In progress', review: 'Review', done: 'Done' },
      empty: 'Nothing here yet',
      more: '+{n} more',
      nothing: 'Press play to watch it unfold.',
      play: 'Play',
      pause: 'Pause',
      again: 'Play again',
      time: 'Time',
      speed: 'Speed',
      figures: 'Figures',
      tasks: 'Tasks',
      landed: 'Landed',
      lines: 'Lines changed',
      agentsTile: 'Agents',
      doneOf: '{n} done',
      sentBack: '{n} sent back, fixed',
      firstTry: 'all on the first try',
      files: { one: '{n} file', other: '{n} files' },
      orchestrators: 'with an orchestrator',
      workers: 'working in parallel',
      shipped: { one: '{agents} shipped {landed} in {duration}.', other: '{agents} shipped {landed} in {duration}.' },
      finished: { one: '{agents} finished {done} in {duration}.', other: '{agents} finished {done} in {duration}.' },
      worked: { one: '{agents} worked on {tasks} for {duration}.', other: '{agents} worked on {tasks} for {duration}.' },
      agentN: { one: '{n} agent', other: '{n} agents' },
      changeN: { one: '{n} change', other: '{n} changes' },
      taskN: { one: '{n} task', other: '{n} tasks' },
      created: 'New task',
      started: '{who} started',
      startedAlone: 'Started',
      progress: '{who} is at {percent} %',
      progressAlone: 'At {percent} %',
      failed: {
        check: 'Sent back: the check failed',
        conflict: 'Sent back: it did not merge cleanly',
        changes: 'Sent back: the review asked for changes',
        other: 'Sent back to try again'
      },
      review: 'Passed, waiting to land',
      landedBy: '{who} landed it',
      landedAlone: 'Landed',
      doneBy: '{who} finished it',
      doneAlone: 'Done',
      checked: 'Check passed',
      reviewed: 'Reviewed',
      back: '{n} sent back',
      waiting: 'Waiting to land',
      orchestrator: 'Orchestrator',
      worker: { one: '{n} task', other: '{n} tasks' },
      landedOf: { one: '{n} landed', other: '{n} landed' },
      working: 'Working',
      idle: 'Idle',
      hidden: { secret: 'hidden', path: 'path', email: 'email', name: 'name', link: 'link' },
      odd: 'This replay could not be read.',
      newer: 'This replay was made by a newer HangarAI. Open it in an up-to-date browser, or ask for a new link.',
      days: { one: '{n} day', other: '{n} days' },
      picture: 'Picture of the delivery'
    },
    fr: {
      eyebrow: 'Replay HangarAI',
      board: 'Le tableau',
      happened: 'Ce qui s’est passé',
      agents: 'Les agents',
      made: 'Fait avec HangarAI',
      try: 'Essayer',
      columns: { todo: 'À faire', doing: 'En cours', review: 'Revue', done: 'Terminé' },
      empty: 'Rien pour l’instant',
      more: '+{n} de plus',
      nothing: 'Lance la lecture pour voir le travail se faire.',
      play: 'Lire',
      pause: 'Pause',
      again: 'Revoir',
      time: 'Temps',
      speed: 'Vitesse',
      figures: 'Chiffres',
      tasks: 'Tâches',
      landed: 'Livrées',
      lines: 'Lignes modifiées',
      agentsTile: 'Agents',
      doneOf: '{n} terminées',
      sentBack: '{n} renvoyées, corrigées',
      firstTry: 'toutes du premier coup',
      files: { one: '{n} fichier', other: '{n} fichiers' },
      orchestrators: 'avec un orchestrateur',
      workers: 'en parallèle',
      shipped: { one: '{agents} a livré {landed} en {duration}.', other: '{agents} ont livré {landed} en {duration}.' },
      finished: { one: '{agents} a terminé {done} en {duration}.', other: '{agents} ont terminé {done} en {duration}.' },
      worked: { one: '{agents} a travaillé sur {tasks} pendant {duration}.', other: '{agents} ont travaillé sur {tasks} pendant {duration}.' },
      agentN: { one: '{n} agent', other: '{n} agents' },
      changeN: { one: '{n} changement', other: '{n} changements' },
      taskN: { one: '{n} tâche', other: '{n} tâches' },
      created: 'Nouvelle tâche',
      started: '{who} a commencé',
      startedAlone: 'Commencée',
      progress: '{who} en est à {percent}\u00a0%',
      progressAlone: 'À {percent}\u00a0%',
      failed: {
        check: 'Renvoyée\u00a0: le contrôle a échoué',
        conflict: 'Renvoyée\u00a0: elle ne fusionnait pas proprement',
        changes: 'Renvoyée\u00a0: la revue a demandé des changements',
        other: 'Renvoyée pour un nouvel essai'
      },
      review: 'Validée, attend d’être intégrée',
      landedBy: '{who} l’a livrée',
      landedAlone: 'Livrée',
      doneBy: '{who} l’a terminée',
      doneAlone: 'Terminée',
      checked: 'Contrôle passé',
      reviewed: 'Relue',
      back: '{n} renvoi(s)',
      waiting: 'Attend d’être intégrée',
      orchestrator: 'Orchestrateur',
      worker: { one: '{n} tâche', other: '{n} tâches' },
      landedOf: { one: '{n} livrée', other: '{n} livrées' },
      working: 'Au travail',
      idle: 'En pause',
      hidden: { secret: 'masqué', path: 'chemin', email: 'e-mail', name: 'nom', link: 'lien' },
      odd: 'Ce replay n’a pas pu être lu.',
      newer: 'Ce replay a été fait par une version plus récente de HangarAI. Ouvre-le dans un navigateur à jour, ou demande un nouveau lien.',
      days: { one: '{n} jour', other: '{n} jours' },
      picture: 'Image de la livraison'
    }
  };

  function fill(text, values) {
    return String(text).replace(/\{(\w+)\}/g, function (all, key) {
      return values && values[key] !== undefined ? values[key] : all;
    });
  }

  function plural(words, n, values) {
    var form = words[n === 1 ? 'one' : 'other'] || words.other;
    var all = { n: n };
    Object.keys(values || {}).forEach(function (k) { all[k] = values[k]; });
    return fill(form, all);
  }

  function number(lang, n) {
    try {
      return new Intl.NumberFormat(lang === 'fr' ? 'fr-FR' : 'en-GB').format(n);
    } catch (e) {
      return String(n);
    }
  }

  // "40 min", "3 h 05", "2 days 4 h".
  function duration(lang, ms) {
    var minutes = Math.max(1, Math.round(ms / 60000));
    if (minutes < 60) return minutes + ' min';
    var hours = Math.floor(minutes / 60);
    if (hours < 24) return hours + ' h ' + String(minutes % 60).padStart(2, '0');
    var days = Math.floor(hours / 24);
    var rest = hours % 24;
    var d = plural(WORDS[lang].days, days);
    return rest ? d + ' ' + rest + ' h' : d;
  }

  // The sentence under the title, in parts: [text, bold?].
  function headline(lang, data) {
    var w = WORDS[lang];
    var t = data.totals;
    var agents = plural(w.agentN, t.agents, { n: number(lang, t.agents) });
    var span = duration(lang, t.duration);
    var form = t.agents === 1 ? 'one' : 'other';
    var template;
    var values = { agents: agents, duration: span };
    if (t.landed > 0) {
      template = w.shipped[form];
      values.landed = plural(w.changeN, t.landed, { n: number(lang, t.landed) });
    } else if (t.done > 0) {
      template = w.finished[form];
      values.done = plural(w.taskN, t.done, { n: number(lang, t.done) });
    } else {
      template = w.worked[form];
      values.tasks = plural(w.taskN, t.tasks, { n: number(lang, t.tasks) });
    }
    var parts = [];
    var re = /\{(\w+)\}/g;
    var last = 0;
    var found;
    while ((found = re.exec(template))) {
      if (found.index > last) parts.push([template.slice(last, found.index), false]);
      parts.push([values[found[1]] !== undefined ? values[found[1]] : found[0], true]);
      last = re.lastIndex;
    }
    if (last < template.length) parts.push([template.slice(last), false]);
    return parts;
  }

  var api = {
    SCHEMA: SCHEMA,
    invalid: invalid,
    positions: positions,
    model: model,
    reached: reached,
    stateAt: stateAt,
    timeAt: timeAt,
    duration: duration,
    headline: headline,
    WORDS: WORDS
  };

  if (typeof document === 'undefined') {
    if (typeof module === 'object' && module.exports) module.exports = api;
    return;
  }

  /* ══ the page ═══════════════════════════════════════════════ */

  var root = document.getElementById('replay');
  var els = {
    title: document.getElementById('rp-title'),
    headline: document.getElementById('rp-headline'),
    when: document.getElementById('rp-when'),
    stats: document.getElementById('rp-stats'),
    board: document.getElementById('rp-board'),
    stage: document.getElementById('rp-stage'),
    play: document.getElementById('rp-play'),
    scrub: document.getElementById('rp-scrub'),
    fill: document.getElementById('rp-fill'),
    ticks: document.getElementById('rp-ticks'),
    clock: document.getElementById('rp-clock'),
    speed: document.getElementById('rp-speed'),
    feed: document.getElementById('rp-feed'),
    agents: document.getElementById('rp-agents'),
    made: document.getElementById('rp-made'),
    trouble: document.getElementById('rp-trouble'),
    live: document.getElementById('rp-live')
  };

  function pickLang() {
    var asked = /(?:^|[#&])lang=(en|fr)\b/.exec(location.hash || '');
    if (asked) return asked[1];
    var nav = (navigator.languages && navigator.languages[0]) || navigator.language || '';
    return /^fr\b/i.test(nav) ? 'fr' : 'en';
  }

  var lang = pickLang();
  var w = WORDS[lang];
  document.documentElement.lang = lang;

  function reducedMotion() {
    return !!(window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches);
  }

  function narrow() {
    return !!(window.matchMedia && window.matchMedia('(max-width: 720px)').matches);
  }

  var framed = false;
  try { framed = window.self !== window.top; } catch (e) { framed = true; }
  if (framed) root.classList.add('is-framed');

  /* ── small builders ────────────────────────────────────────── */

  function el(tag, cls, text) {
    var node = document.createElement(tag);
    if (cls) node.className = cls;
    if (text !== undefined && text !== null) node.textContent = text;
    return node;
  }

  // Text with the redaction's placeholders drawn as small marks.
  function rich(node, text) {
    node.textContent = '';
    var last = 0;
    var found;
    HIDDEN.lastIndex = 0;
    while ((found = HIDDEN.exec(text))) {
      if (found.index > last) node.appendChild(document.createTextNode(text.slice(last, found.index)));
      var mark = el('span', 'rp-hidden', w.hidden[found[1]]);
      mark.title = w.hidden[found[1]];
      node.appendChild(mark);
      last = HIDDEN.lastIndex;
    }
    if (last < text.length) node.appendChild(document.createTextNode(text.slice(last)));
    return node;
  }

  function hue(name) {
    var i = name in M.agentIndex ? M.agentIndex[name] : 0;
    return 'var(--a' + i + ')';
  }

  function who(name) {
    var span = el('span', 'rp-who');
    span.style.setProperty('--agent', hue(name));
    span.appendChild(el('span', 'rp-who__dot'));
    span.appendChild(el('span', 'rp-who__name', name));
    return span;
  }

  function formatter(options) {
    try {
      return new Intl.DateTimeFormat(lang === 'fr' ? 'fr-FR' : 'en-GB', options);
    } catch (e) {
      return { format: function (d) { return d.toISOString().slice(0, 16).replace('T', ' '); } };
    }
  }

  var dayFmt = formatter({ day: 'numeric', month: 'long', year: 'numeric' });
  var timeFmt = formatter({ hour: '2-digit', minute: '2-digit' });
  var shortFmt = formatter({ day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' });

  function sameDay(a, b) {
    var x = new Date(a);
    var y = new Date(b);
    return x.getFullYear() === y.getFullYear() && x.getMonth() === y.getMonth() && x.getDate() === y.getDate();
  }

  /* ── the parts that do not move ────────────────────────────── */

  var M = null;
  var multiDay = false;

  function head() {
    var data = M.data;
    els.title.textContent = '';
    rich(els.title, data.title);
    document.title = data.title + ' · HangarAI';
    els.headline.textContent = '';
    headline(lang, data).forEach(function (part) {
      if (part[1]) els.headline.appendChild(el('b', null, part[0]));
      else els.headline.appendChild(document.createTextNode(part[0]));
    });
    var from = new Date(data.from);
    var to = new Date(data.to);
    multiDay = !sameDay(data.from, data.to);
    els.when.textContent = multiDay
      ? dayFmt.format(from) + ' – ' + dayFmt.format(to)
      : dayFmt.format(from) + ' · ' + timeFmt.format(from) + ' – ' + timeFmt.format(to);
    Array.prototype.forEach.call(document.querySelectorAll('[data-w]'), function (node) {
      var word = w[node.getAttribute('data-w')];
      if (typeof word === 'string') node.textContent = word;
    });
    els.play.setAttribute('aria-label', w.play);
    els.scrub.setAttribute('aria-label', w.time);
    els.speed.setAttribute('aria-label', w.speed);
    els.stats.setAttribute('aria-label', w.figures);
    if (data.referral) {
      els.made.href = 'https://hangarai.dev/r/' + encodeURIComponent(data.referral);
    }
    ticks();
  }

  function ticks() {
    els.ticks.textContent = '';
    M.events.forEach(function (ev, i) {
      if (ev.type === 'progress' || ev.type === 'created') return;
      var tick = el('span', 'rp-tick rp-tick--' + ev.type);
      tick.style.left = (M.at[i] * 100).toFixed(3) + '%';
      els.ticks.appendChild(tick);
    });
  }

  /* ── the figures ───────────────────────────────────────────── */

  var tiles = null;

  function tile(label) {
    var node = el('div', 'rp-stat');
    node.appendChild(el('span', 'rp-stat__label', label));
    var value = el('span', 'rp-stat__value');
    var sub = el('span', 'rp-stat__sub');
    node.appendChild(value);
    node.appendChild(sub);
    els.stats.appendChild(node);
    return { value: value, sub: sub };
  }

  function stats(state) {
    var t = M.data.totals;
    if (!tiles) {
      els.stats.textContent = '';
      tiles = {
        tasks: tile(w.tasks),
        landed: tile(w.landed),
        lines: tile(w.lines),
        agents: tile(w.agentsTile)
      };
    }
    setFigure(tiles.tasks.value, number(lang, state.shown), number(lang, t.tasks));
    tiles.tasks.sub.textContent = fill(w.doneOf, { n: number(lang, state.done) });
    setFigure(tiles.landed.value, number(lang, state.counts.landed), null);
    tiles.landed.sub.textContent = state.counts.failed
      ? fill(w.sentBack, { n: number(lang, state.counts.failed) })
      : (state.counts.landed ? w.firstTry : '');
    var lines = tiles.lines.value;
    lines.textContent = '';
    lines.appendChild(el('span', 'rp-plus', '+' + number(lang, state.counts.added)));
    lines.appendChild(document.createTextNode(' '));
    lines.appendChild(el('span', 'rp-minus', '−' + number(lang, state.counts.removed)));
    tiles.lines.sub.textContent = plural(w.files, state.counts.files, { n: number(lang, state.counts.files) });
    setFigure(tiles.agents.value, number(lang, t.agents), null);
    var orchestrated = M.data.agents.some(function (a) { return a.role === 'orchestrator'; });
    tiles.agents.sub.textContent = orchestrated ? w.orchestrators : w.workers;
  }

  function setFigure(node, value, of) {
    node.textContent = value;
    if (of !== null && of !== value) {
      node.appendChild(document.createTextNode(' '));
      node.appendChild(el('span', 'rp-stat__of', '/ ' + of));
    }
  }

  /* ── the board ─────────────────────────────────────────────── */

  var cardEls = {};
  var columnEls = null;

  function columns() {
    els.board.textContent = '';
    columnEls = {};
    COLUMNS.forEach(function (name) {
      var col = el('section', 'rp-col');
      col.setAttribute('data-col', name);
      var headEl = el('div', 'rp-col__head');
      headEl.appendChild(el('span', 'rp-col__dot'));
      var label = el('span', null, w.columns[name]);
      headEl.appendChild(label);
      var count = el('span', 'rp-col__count', '0');
      headEl.appendChild(count);
      col.appendChild(headEl);
      var list = el('ol', 'rp-col__cards');
      col.appendChild(list);
      var more = el('p', 'rp-col__more');
      more.hidden = true;
      col.appendChild(more);
      var empty = el('p', 'rp-col__empty', w.empty);
      col.appendChild(empty);
      col.setAttribute('aria-label', w.columns[name]);
      els.board.appendChild(col);
      columnEls[name] = { col: col, list: list, count: count, more: more, empty: empty };
    });
  }

  function cardEl(id) {
    if (cardEls[id]) return cardEls[id];
    var card = M.cards[id].card;
    var node = el('li', 'rp-card');
    var title = el('p', 'rp-card__title');
    rich(title, card.title);
    node.appendChild(title);
    var meta = el('div', 'rp-card__meta');
    node.appendChild(meta);
    var bar = el('div', 'rp-bar');
    var barFill = el('div', 'rp-bar__fill');
    bar.appendChild(barFill);
    node.appendChild(bar);
    cardEls[id] = { node: node, meta: meta, bar: bar, barFill: barFill, key: '' };
    return cardEls[id];
  }

  function paintCard(c, task) {
    var card = M.cards[task.id].card;
    var key = [task.column, task.agent, task.percent, task.back, task.landed ? 1 : 0].join('|');
    if (c.key === key) return false;
    c.key = key;
    c.meta.textContent = '';
    if (task.agent) c.meta.appendChild(who(task.agent));
    (card.labels || []).slice(0, 2).forEach(function (label) {
      c.meta.appendChild(rich(el('span', 'rp-tag'), label));
    });
    var status = null;
    if (task.column === 'done' && task.landed) {
      status = el('span', 'rp-card__status rp-card__status--landed');
      status.appendChild(checkIcon());
      if (task.landed.added !== undefined) {
        status.appendChild(el('span', 'rp-plus', '+' + number(lang, task.landed.added)));
        status.appendChild(el('span', 'rp-minus', '−' + number(lang, task.landed.removed || 0)));
      }
    } else if (task.column === 'done') {
      status = el('span', 'rp-card__status rp-card__status--landed');
      status.appendChild(checkIcon());
    } else if (task.column === 'review') {
      status = el('span', 'rp-card__status rp-card__status--review', w.waiting);
    } else if (task.back) {
      status = el('span', 'rp-card__status rp-card__status--back', '↺ ' + task.back);
      status.title = fill(w.back, { n: task.back });
    }
    if (status) c.meta.appendChild(status);
    var working = task.column === 'doing';
    c.node.classList.toggle('is-working', working);
    c.bar.hidden = !(working && task.percent !== null);
    c.barFill.style.width = (task.percent || 0) + '%';
    return true;
  }

  function checkIcon() {
    var svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.setAttribute('width', '12');
    svg.setAttribute('height', '12');
    svg.setAttribute('viewBox', '0 0 16 16');
    svg.setAttribute('aria-hidden', 'true');
    var path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    path.setAttribute('d', 'm3.5 8.5 3 3 6-7');
    path.setAttribute('fill', 'none');
    path.setAttribute('stroke', 'currentColor');
    path.setAttribute('stroke-width', '1.9');
    path.setAttribute('stroke-linecap', 'round');
    path.setAttribute('stroke-linejoin', 'round');
    svg.appendChild(path);
    return svg;
  }

  // How many cards a column shows; the rest is "+N more".
  var cardsShown = 6;

  function limit() {
    return cardsShown;
  }

  // As many cards a column as let the board and its timeline fit in the
  // window from the top of the page, so that the timeline is in sight and
  // nothing ever lies over the board: measured on the fullest board, the
  // end's, from 6 (4 on a phone) down to 2 (1 on a phone, whose board is
  // two rows of two). A window too short for that scrolls, as any page
  // does.
  function fit() {
    var most = narrow() ? 4 : 6;
    var least = narrow() ? 1 : 2;
    var room = window.innerHeight || document.documentElement.clientHeight || 0;
    var fullest = stateAt(M, M.events.length);
    var top = els.stage.getBoundingClientRect().top + (window.scrollY || 0);
    cardsShown = most;
    els.board.style.minHeight = '0px';
    board(fullest, false);
    while (cardsShown > least && room > 0 && top + els.stage.offsetHeight > room - 12) {
      cardsShown -= 1;
      board(fullest, false);
    }
    // The board keeps that height while it plays: the timeline under it
    // stays where it is as cards come and go.
    els.board.style.minHeight = els.board.offsetHeight + 'px';
    count = -1;
  }

  function board(state, animate) {
    var motion = animate && !reducedMotion() && typeof Element.prototype.animate === 'function';
    var before = {};
    if (motion) {
      Object.keys(cardEls).forEach(function (id) {
        var node = cardEls[id].node;
        if (node.isConnected) before[id] = node.getBoundingClientRect();
      });
    }
    var shown = {};
    var changed = {};
    COLUMNS.forEach(function (name) {
      var parts = columnEls[name];
      var list = state.columns[name];
      var max = limit();
      parts.count.textContent = number(lang, list.length);
      parts.empty.hidden = list.length > 0;
      parts.more.hidden = list.length <= max;
      parts.more.textContent = fill(w.more, { n: number(lang, list.length - max) });
      var at = 0;
      list.slice(0, max).forEach(function (task) {
        var c = cardEl(task.id);
        if (paintCard(c, task)) changed[task.id] = true;
        var here = parts.list.children[at];
        if (here !== c.node) parts.list.insertBefore(c.node, here || null);
        shown[task.id] = true;
        at++;
      });
    });
    Object.keys(cardEls).forEach(function (id) {
      if (!shown[id] && cardEls[id].node.isConnected) cardEls[id].node.remove();
    });
    if (!motion) return;
    Object.keys(shown).forEach(function (id) {
      var node = cardEls[id].node;
      var after = node.getBoundingClientRect();
      var was = before[id];
      if (!was) {
        node.animate(
          [{ opacity: 0, transform: 'translateY(-6px) scale(0.97)' }, { opacity: 1, transform: 'none' }],
          { duration: 360, easing: 'cubic-bezier(0.2, 0.7, 0.2, 1)' }
        );
        return;
      }
      var dx = was.left - after.left;
      var dy = was.top - after.top;
      if (Math.abs(dx) < 1 && Math.abs(dy) < 1) return;
      var across = Math.abs(dx) > 4;
      node.animate(
        across
          ? [
              { transform: 'translate(' + dx + 'px,' + dy + 'px)', boxShadow: 'var(--shadow-lift)', zIndex: 2 },
              { transform: 'translate(' + (dx * 0.5) + 'px,' + (dy * 0.5 - 10) + 'px) scale(1.02)', boxShadow: 'var(--shadow-lift)', zIndex: 2, offset: 0.5 },
              { transform: 'none', zIndex: 2 }
            ]
          : [{ transform: 'translate(' + dx + 'px,' + dy + 'px)' }, { transform: 'none' }],
        { duration: across ? 620 : 380, easing: 'cubic-bezier(0.2, 0.7, 0.2, 1)' }
      );
      if (across && state.tasks[id].column === 'done' && changed[id]) {
        node.classList.remove('is-flash');
        void node.offsetWidth;
        node.classList.add('is-flash');
      }
    });
  }

  /* ── what happened ─────────────────────────────────────────── */

  var FEED = 6;
  var feedKey = '';

  function feedLine(ev) {
    var task = M.cards[ev.task].card;
    var line = el('p', 'rp-item__line');
    var put = function (template, name) {
      var bits = template.split('{who}');
      if (bits[0]) line.appendChild(document.createTextNode(bits[0]));
      if (bits.length > 1) {
        line.appendChild(who(name));
        line.appendChild(document.createTextNode(bits[1]));
      }
    };
    switch (ev.type) {
      case 'created': line.appendChild(document.createTextNode(w.created)); break;
      case 'started': put(ev.agent ? w.started : w.startedAlone, ev.agent); break;
      case 'progress': put(fill(ev.agent ? w.progress : w.progressAlone, { percent: ev.percent }), ev.agent); break;
      case 'failed': line.appendChild(document.createTextNode(w.failed[ev.why || 'other'])); break;
      case 'review': line.appendChild(document.createTextNode(w.review)); break;
      case 'landed': if (ev.agent) put(w.landedBy, ev.agent); else line.appendChild(document.createTextNode(w.landedAlone)); break;
      case 'done': if (ev.agent) put(w.doneBy, ev.agent); else line.appendChild(document.createTextNode(w.doneAlone)); break;
    }
    line.appendChild(document.createTextNode(' · '));
    line.appendChild(rich(el('span', 'rp-item__task'), task.title));
    return line;
  }

  function feedItem(ev, fresh) {
    var item = el('li', 'rp-item');
    if (fresh) item.classList.add('is-in');
    var time = new Date(ev.at);
    item.appendChild(el('span', 'rp-item__time', multiDay ? shortFmt.format(time) : timeFmt.format(time)));
    var body = el('div', 'rp-item__body');
    body.appendChild(feedLine(ev));
    var text = ev.type === 'landed' ? ev.summary : ev.type === 'progress' ? ev.note : null;
    if (text) body.appendChild(rich(el('p', 'rp-item__summary'), text));
    if (ev.type === 'landed') {
      var chips = el('div', 'rp-item__chips');
      if (ev.added !== undefined) {
        var change = el('span', 'rp-chip');
        change.appendChild(el('span', 'rp-plus', '+' + number(lang, ev.added)));
        change.appendChild(el('span', 'rp-minus', '−' + number(lang, ev.removed || 0)));
        change.appendChild(document.createTextNode(' · ' + plural(w.files, ev.files || 0, { n: number(lang, ev.files || 0) })));
        chips.appendChild(change);
      }
      if (ev.checked) chips.appendChild(el('span', 'rp-chip rp-chip--ok', '✓ ' + w.checked));
      if (ev.reviewed) chips.appendChild(el('span', 'rp-chip rp-chip--ok', '✓ ' + w.reviewed));
      if (ev.commit) chips.appendChild(el('span', 'rp-chip rp-chip--mono', ev.commit.slice(0, 7)));
      if (chips.children.length) body.appendChild(chips);
      if (ev.pictures && ev.pictures.length) {
        var shots = el('div', 'rp-shots');
        ev.pictures.forEach(function (path) {
          if (!SHOT.test(path)) return;
          var frame = el('span', 'rp-shot');
          var img = document.createElement('img');
          img.loading = 'lazy';
          img.decoding = 'async';
          img.alt = w.picture;
          img.src = path;
          frame.appendChild(img);
          shots.appendChild(frame);
        });
        body.appendChild(shots);
      }
    }
    item.appendChild(body);
    return item;
  }

  function feed(count, animate) {
    var from = Math.max(0, count - FEED);
    var key = from + ':' + count;
    if (key === feedKey) return;
    var previous = feedKey ? Number(feedKey.split(':')[1]) : 0;
    feedKey = key;
    els.feed.textContent = '';
    if (!count) {
      els.feed.appendChild(el('li', 'rp-feed__empty', w.nothing));
      return;
    }
    for (var i = count - 1; i >= from; i--) {
      els.feed.appendChild(feedItem(M.events[i], animate && i >= previous && !reducedMotion()));
    }
  }

  /* ── the agents ────────────────────────────────────────────── */

  var agentEls = {};

  function agents(state) {
    if (!Object.keys(agentEls).length) {
      els.agents.textContent = '';
      M.data.agents.forEach(function (agent) {
        var item = el('li', 'rp-agent');
        item.style.setProperty('--agent', hue(agent.name));
        item.appendChild(el('span', 'rp-avatar', (agent.name.trim()[0] || '?').toUpperCase()));
        var text = el('span', 'rp-agent__text');
        text.appendChild(el('span', 'rp-agent__name', agent.name));
        var what = agent.role === 'orchestrator'
          ? w.orchestrator
          : plural(w.worker, agent.tasks, { n: number(lang, agent.tasks) }) +
            (agent.landed ? ' · ' + plural(w.landedOf, agent.landed, { n: number(lang, agent.landed) }) : '');
        text.appendChild(el('span', 'rp-agent__what', what));
        item.appendChild(text);
        var stateEl = el('span', 'rp-agent__state');
        item.appendChild(stateEl);
        els.agents.appendChild(item);
        agentEls[agent.name] = { item: item, state: stateEl };
      });
    }
    M.data.agents.forEach(function (agent) {
      var parts = agentEls[agent.name];
      var working = !!state.working[agent.name];
      parts.item.classList.toggle('is-working', working);
      parts.state.textContent = working ? w.working : '';
    });
  }

  /* ── the player ────────────────────────────────────────────── */

  var s = 0;
  var count = -1;
  var playing = false;
  var speed = 1;
  var lastFrame = 0;
  var length = 20000;

  function show(animate) {
    var n = reached(M, s);
    var state = stateAt(M, n);
    if (n !== count) {
      board(state, animate);
      feed(n, animate);
      stats(state);
      agents(state);
      if (playing && n > count && count >= 0) {
        var ev = M.events[n - 1];
        if (ev.type === 'landed') {
          els.live.textContent = M.cards[ev.task].card.title;
        }
      }
      count = n;
    }
    var value = Math.round(s * 1000);
    if (Number(els.scrub.value) !== value) els.scrub.value = String(value);
    els.fill.style.width = (s * 100).toFixed(2) + '%';
    var at = timeAt(M, s);
    var since = at - M.data.from;
    els.clock.textContent = '';
    els.clock.appendChild(el('b', null, multiDay ? shortFmt.format(new Date(at)) : timeFmt.format(new Date(at))));
    els.clock.appendChild(document.createTextNode('+' + (since < 30000 ? '0 min' : duration(lang, since))));
    els.scrub.setAttribute('aria-valuetext', els.clock.textContent);
    button();
  }

  function button() {
    var mode = playing ? 'pause' : s >= 1 ? 'again' : 'play';
    els.play.setAttribute('data-mode', mode);
    els.play.setAttribute('aria-label', mode === 'pause' ? w.pause : mode === 'again' ? w.again : w.play);
  }

  function frame(now) {
    if (!playing) return;
    var dt = lastFrame ? Math.min(now - lastFrame, 100) : 16;
    lastFrame = now;
    s = Math.min(1, s + (dt / length) * speed);
    show(true);
    if (s >= 1) {
      playing = false;
      show(false);
      return;
    }
    requestAnimationFrame(frame);
  }

  function play() {
    if (playing) return;
    if (s >= 1) {
      s = 0;
      count = -1;
      show(false);
    }
    playing = true;
    lastFrame = 0;
    button();
    requestAnimationFrame(frame);
  }

  function pause() {
    playing = false;
    show(false);
  }

  // To the next event, or back to the one before the last that happened.
  function step(dir) {
    pause();
    var n = reached(M, s);
    if (dir > 0) s = n < M.at.length ? M.at[n] : 1;
    else s = n >= 2 ? M.at[n - 2] : 0;
    show(true);
  }

  els.play.addEventListener('click', function () {
    if (playing) pause();
    else play();
  });

  els.scrub.addEventListener('input', function () {
    playing = false;
    s = Number(els.scrub.value) / 1000;
    show(false);
  });

  els.scrub.addEventListener('keydown', function (event) {
    if (event.key === 'ArrowRight' || event.key === 'ArrowUp') { event.preventDefault(); step(1); }
    else if (event.key === 'ArrowLeft' || event.key === 'ArrowDown') { event.preventDefault(); step(-1); }
    else if (event.key === 'Home') { event.preventDefault(); pause(); s = 0; show(false); }
    else if (event.key === 'End') { event.preventDefault(); pause(); s = 1; show(false); }
  });

  document.addEventListener('keydown', function (event) {
    if (!M || event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) return;
    var tag = (event.target && event.target.tagName) || '';
    if (event.key === ' ' && tag !== 'BUTTON' && tag !== 'A') {
      event.preventDefault();
      if (playing) pause();
      else play();
    }
  });

  els.speed.addEventListener('click', function (event) {
    var pick = event.target.closest('button[data-speed]');
    if (!pick) return;
    speed = Number(pick.getAttribute('data-speed')) || 1;
    Array.prototype.forEach.call(els.speed.querySelectorAll('button'), function (b) {
      b.setAttribute('aria-pressed', String(b === pick));
    });
  });

  var resized = 0;
  window.addEventListener('resize', function () {
    clearTimeout(resized);
    resized = setTimeout(function () {
      if (!M) return;
      fit();
      show(false);
    }, 120);
  });

  /* ── start ─────────────────────────────────────────────────── */

  function trouble(text) {
    root.setAttribute('data-state', 'error');
    root.setAttribute('aria-busy', 'false');
    els.trouble.hidden = false;
    els.trouble.textContent = text;
  }

  function start(data) {
    var why = invalid(data);
    if (why) {
      trouble(data && data.v > SCHEMA ? w.newer : w.odd);
      if (window.console) console.warn('replay: ' + why);
      return;
    }
    M = model(data);
    // A page made for one of these, framed or alone: the time it takes
    // at 1×, about two thirds of a second an event, 12 s to 48 s.
    length = Math.min(48000, Math.max(12000, M.events.length * 650));
    head();
    columns();
    root.setAttribute('data-state', 'ready');
    root.setAttribute('aria-busy', 'false');
    fit();
    // The page's font may come in after the first layout: measured again.
    if (document.fonts && document.fonts.ready) {
      document.fonts.ready.then(function () {
        fit();
        show(false);
      });
    }
    // Someone who asked for less motion gets the end, still; anyone else
    // the beginning, then it plays once.
    var hash = location.hash || '';
    var at = /(?:^|[#&])at=(end|start|\d{1,3})\b/.exec(hash);
    if (reducedMotion() || (at && at[1] === 'end')) {
      s = 1;
      show(false);
      return;
    }
    if (at && /^\d+$/.test(at[1])) {
      s = Math.min(1, Number(at[1]) / 100);
      show(false);
      return;
    }
    s = 0;
    show(false);
    setTimeout(function () {
      if (document.visibilityState === 'hidden') {
        document.addEventListener('visibilitychange', function once() {
          if (document.visibilityState !== 'visible') return;
          document.removeEventListener('visibilitychange', once);
          play();
        });
        return;
      }
      play();
    }, 900);
  }

  var holder = document.getElementById('replay-data');
  var data = null;
  try {
    data = JSON.parse(holder.textContent);
  } catch (e) {
    data = undefined;
  }
  if (data === null && /^https?:$/.test(location.protocol)) {
    // The template, opened as it is while it is worked on.
    fetch('sample.json', { cache: 'no-store' })
      .then(function (res) { return res.json(); })
      .then(start, function () { trouble(w.odd); });
  } else {
    start(data);
  }
})();
