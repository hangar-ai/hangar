/* ═══════════════════════════════════════════════════════════════
   HangarAI site: the share viewer, hangarai.dev/s/<id> (s/index.html)

   The id is the page's own path (sws.toml rewrites every /s/* to
   s/index.html). The share's metadata comes from the account server,
   GET https://account.hangarai.dev/v1/shares/<id>, without a cookie
   or a token: the title, the kind, the files, where they are served,
   until when (docs/native/cloud-services.md, "5. Share links";
   hangar-cloud-api's shares.rs, PublicShare). An ended share answers
   410 with how it ended, and nothing of it is shown.

   The files are never served by this origin: they come from
   https://share.hangarai.dev/<id>/…, a Cloudflare custom domain on the
   `hangarai-shares` bucket whose every answer carries a sandboxing
   Content-Security-Policy (docs/native/shares.md). This page frames a
   shared page with the same sandbox, so that nothing of hangarai.dev,
   its cookies or its scripts, is ever within a shared page's reach.

   Nothing here measures a visitor. "Report this page" posts a reason
   and the visitor's words, if any, to /v1/shares/<id>/report: never
   who they are.
   ═══════════════════════════════════════════════════════════════ */
(function () {
  'use strict';

  var SERVER = 'https://account.hangarai.dev';
  var CONTENT = 'https://share.hangarai.dev';
  var MODEL_VIEWER = '/assets/model-viewer/model-viewer.min.js?v=c333415cbd';
  var ID = /^[A-Za-z0-9_-]{22}$/;
  // This computer: a loopback address, or a name under .localhost, which
  // browsers always resolve to it (RFC 6761).
  var LOCAL_HOST = '(?:127\\.0\\.0\\.1|\\[::1\\]|localhost|(?:[a-z0-9-]+\\.)+localhost)';
  var LOOPBACK = new RegExp('^' + LOCAL_HOST + '$');
  var LOCAL_SERVER = new RegExp('^http://' + LOCAL_HOST + ':\\d{2,5}$');
  var LOCAL_BASE = new RegExp('^(http://' + LOCAL_HOST + ':\\d{2,5})/([A-Za-z0-9_-]{22})/$');
  var KINDS = ['page', 'markdown', 'pdf', 'image', 'model'];
  // A replay is a page the app made (s/replay/): its entry says so.
  var REPLAY_ENTRY = /(^|\/)replay\.html$/;
  // A referral code, as a replay's link carries it (?ref=…): 3 to 20 of
  // [a-z0-9-], no dash at either end or twice (hangar-cloud-api's is_code).
  var REF = /^[a-z0-9](?:[a-z0-9]|-(?!-)){1,18}[a-z0-9]$/;

  var WORDS = {
    en: {
      kind: { page: 'Page', markdown: 'Document', pdf: 'PDF', image: 'Image', model: '3D model', replay: 'Replay' },
      shared: 'Shared with HangarAI',
      until: 'until {date}',
      offline: 'This device could not reach HangarAI’s server. Check the connection and try again.',
      busy: 'HangarAI’s server asks to wait a moment. Try again in a minute.',
      server: 'HangarAI’s server answered with an error ({status}). Try again in a moment.',
      odd: 'HangarAI’s server gave an answer this page does not understand. Reload it, or try again later.',
      newer: 'This kind of share needs a newer version of this page. Reload it.',
      image: 'The image could not be loaded. Reload the page to try again.',
      model: 'The 3D model could not be loaded. Reload the page to try again.',
      viewer: 'The 3D viewer could not start in this browser.',
      turn: 'Drag to turn · scroll to zoom',
      turnTouch: 'Drag to turn · pinch to zoom',
      thanks: 'Thank you: your report has been sent. HangarAI looks at it within the hour and takes the page down if it breaks the rules.',
      reportBusy: 'Too many reports from this connection for now. Write to {abuse} instead.',
      reportGone: 'This link has just ended: there is nothing left to report.',
      reportRefused: 'The report was refused: {why}',
      reportFailed: 'The report could not be sent. Try again, or write to {abuse}.',
      loaded: '{title}, shared with HangarAI, is shown.'
    },
    fr: {
      kind: { page: 'Page', markdown: 'Document', pdf: 'PDF', image: 'Image', model: 'Modèle 3D', replay: 'Replay' },
      shared: 'Partagé avec HangarAI',
      until: 'jusqu’au {date}',
      offline: 'Cet appareil n’a pas pu joindre le serveur de HangarAI. Vérifie la connexion et réessaie.',
      busy: 'Le serveur de HangarAI demande d’attendre un instant. Réessaie dans une minute.',
      server: 'Le serveur de HangarAI a répondu par une erreur ({status}). Réessaie dans un instant.',
      odd: 'Le serveur de HangarAI a donné une réponse que cette page ne comprend pas. Recharge-la, ou réessaie plus tard.',
      newer: 'Ce genre de partage demande une version plus récente de cette page. Recharge-la.',
      image: 'L’image n’a pas pu être chargée. Recharge la page pour réessayer.',
      model: 'Le modèle 3D n’a pas pu être chargé. Recharge la page pour réessayer.',
      viewer: 'La visionneuse 3D n’a pas pu démarrer dans ce navigateur.',
      turn: 'Fais glisser pour tourner · fais défiler pour zoomer',
      turnTouch: 'Fais glisser pour tourner · pince pour zoomer',
      thanks: 'Merci\u00a0: ton signalement a été envoyé. HangarAI l’examine dans l’heure et retire la page si elle enfreint les règles.',
      reportBusy: 'Trop de signalements depuis cette connexion pour l’instant. Écris plutôt à {abuse}.',
      reportGone: 'Ce lien vient de prendre fin\u00a0: il n’y a plus rien à signaler.',
      reportRefused: 'Le signalement a été refusé\u00a0: {why}',
      reportFailed: 'Le signalement n’a pas pu être envoyé. Réessaie, ou écris à {abuse}.',
      loaded: '{title}, partagé avec HangarAI, est affiché.'
    }
  };

  var ICONS = {
    page: '<svg width="14" height="14" viewBox="0 0 16 16"><rect x="2.25" y="2.75" width="11.5" height="10.5" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M2.5 5.75h11" stroke="currentColor" stroke-width="1.3"/></svg>',
    markdown: '<svg width="14" height="14" viewBox="0 0 16 16"><path d="M4 1.75h5.5L12.75 5v8.25a1 1 0 0 1-1 1h-7.75a1 1 0 0 1-1-1V2.75a1 1 0 0 1 1-1Z M9.25 2v3.25h3.25M5.5 8.5h5M5.5 11h3.5" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round" stroke-linecap="round"/></svg>',
    pdf: '<svg width="14" height="14" viewBox="0 0 16 16"><path d="M4 1.75h5.5L12.75 5v8.25a1 1 0 0 1-1 1h-7.75a1 1 0 0 1-1-1V2.75a1 1 0 0 1 1-1Z M9.25 2v3.25h3.25" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/></svg>',
    image: '<svg width="14" height="14" viewBox="0 0 16 16"><rect x="2" y="2.75" width="12" height="10.5" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.3"/><circle cx="5.75" cy="6.25" r="1.1" fill="currentColor"/><path d="m2.5 11.5 3.25-3 2.5 2.25 2-1.75 3.25 2.75" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/></svg>',
    model: '<svg width="14" height="14" viewBox="0 0 16 16"><path d="M8 1.75 13.5 4.8v6.4L8 14.25 2.5 11.2V4.8L8 1.75Zm0 0v0M2.75 4.95 8 8l5.25-3.05M8 8v6" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/></svg>',
    replay: '<svg width="14" height="14" viewBox="0 0 16 16"><circle cx="8" cy="8" r="6.25" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M6.6 5.6v4.8L10.4 8 6.6 5.6Z" fill="currentColor"/></svg>'
  };

  var root = document.documentElement;
  var main = document.getElementById('main');
  var stage = document.getElementById('share-stage');
  var titleEl = document.getElementById('share-title');
  var metaEl = document.getElementById('share-meta');
  var kindEl = document.getElementById('share-kind');
  var troubleEl = document.getElementById('share-trouble');
  var fullBtn = document.getElementById('share-full');
  var openLink = document.getElementById('share-open');
  var reportBtn = document.getElementById('share-report');
  var live = document.getElementById('live');

  var state = { name: 'loading', reason: null, share: null, fault: null, trouble: null };

  function lang() {
    return root.dataset.lang === 'fr' ? 'fr' : 'en';
  }

  function word(key, fill) {
    var text = WORDS[lang()][key] || WORDS.en[key] || '';
    Object.keys(fill || {}).forEach(function (name) {
      text = text.split('{' + name + '}').join(fill[name]);
    });
    return text;
  }

  /* ── where things are ──────────────────────────────────────── */

  var onLoopback = LOOPBACK.test(location.hostname);

  // A page served from this computer may name a local account server
  // (?api=http://127.0.0.1:<port>, kept for the tab), as the account pages
  // do (assets/api.js); anywhere else it is the real one.
  function server() {
    if (!onLoopback) return SERVER;
    var asked = null;
    try { asked = new URLSearchParams(location.search).get('api'); } catch (e) { /* old browser */ }
    try {
      if (asked && LOCAL_SERVER.test(asked)) sessionStorage.setItem('hangar.api', asked);
      var kept = sessionStorage.getItem('hangar.api');
      if (kept && LOCAL_SERVER.test(kept)) return kept;
    } catch (e) { /* storage refused */ }
    return asked && LOCAL_SERVER.test(asked) ? asked : SERVER;
  }

  // The share's id, from /s/<id>; null for /s/ itself.
  function shareId() {
    var match = /^\/s\/([^/?#]*)\/?$/.exec(location.pathname);
    var id = match ? match[1] : '';
    if (!id || id === 'index.html') return null;
    return id;
  }

  // Where the server says the files are: on the shares' origin and under
  // the share's own folder, or nowhere (a local stand-in only on a
  // loopback page).
  function contentBase(share) {
    var base = String(share.content_base || '');
    var expected = CONTENT + '/' + share.id + '/';
    if (base === expected) return base;
    if (onLoopback) {
      var local = LOCAL_BASE.exec(base);
      if (local && local[2] === share.id) return base;
    }
    return null;
  }

  function fileUrl(base, path) {
    return base + String(path).split('/').map(encodeURIComponent).join('/');
  }

  /* ── the page's states ─────────────────────────────────────── */

  function shows(list) {
    return list.split(' ').indexOf(state.name) !== -1;
  }

  function render() {
    main.dataset.state = state.name;
    main.setAttribute('aria-busy', String(state.name === 'loading'));
    Array.prototype.forEach.call(document.querySelectorAll('[data-show]'), function (el) {
      var on = shows(el.dataset.show);
      if (on && el.dataset.showReason) on = el.dataset.showReason.split(' ').indexOf(state.reason) !== -1;
      el.hidden = !on;
    });
    Array.prototype.forEach.call(document.querySelectorAll('[data-show-icon]'), function (el) {
      var icon = state.name === 'gone' ? (state.reason === 'taken_down' ? 'taken_down' : 'gone')
        : state.name === 'none' ? 'missing' : state.name;
      el.style.display = el.dataset.showIcon === icon ? '' : 'none';
    });
    if (state.name === 'error') {
      document.getElementById('share-error-text').textContent = state.fault ? word(state.fault.key, state.fault.fill) : '';
    }
    if (state.name === 'live' && state.share) head(state.share);
    troubleEl.hidden = !state.trouble;
    troubleEl.textContent = state.trouble ? word(state.trouble) : '';
  }

  // What the head and the stage call a share: a replay, or its kind.
  function shownKind(share) {
    return share.kind === 'page' && REPLAY_ENTRY.test(share.entry) ? 'replay' : share.kind;
  }

  function head(share) {
    var kind = shownKind(share);
    kindEl.querySelector('.share__kind-icon').innerHTML = ICONS[kind] || '';
    kindEl.querySelector('.share__kind-name').textContent = (WORDS[lang()].kind[kind]) || '';
    titleEl.textContent = share.title;
    document.title = share.title + ' · HangarAI';
    var date = new Date(share.expires_at);
    var day = '';
    try {
      day = new Intl.DateTimeFormat(lang() === 'fr' ? 'fr-FR' : 'en-GB', { day: 'numeric', month: 'long', year: 'numeric' }).format(date);
    } catch (e) {
      day = date.toDateString();
    }
    metaEl.textContent = '';
    var shared = document.createElement('span');
    shared.textContent = word('shared');
    var dot = document.createElement('span');
    dot.className = 'dot';
    dot.setAttribute('aria-hidden', 'true');
    dot.textContent = '·';
    var until = document.createElement('time');
    until.dateTime = date.toISOString();
    until.textContent = word('until', { date: day });
    metaEl.appendChild(shared);
    metaEl.appendChild(dot);
    metaEl.appendChild(until);
    var hint = stage.querySelector('.share__hint');
    if (hint) hint.textContent = touch() ? word('turnTouch') : word('turn');
  }

  function set(name, extra) {
    state.name = name;
    state.reason = (extra && extra.reason) || null;
    state.fault = (extra && extra.fault) || null;
    render();
    if (name !== 'loading' && name !== 'live') {
      var heading = document.querySelector('.share__end [data-show]:not([hidden]) .share__end-title');
      if (heading && document.activeElement === document.body) heading.focus({ preventScroll: true });
    }
  }

  /* ── what was shared, drawn ────────────────────────────────── */

  function touch() {
    return window.matchMedia && window.matchMedia('(pointer: coarse)').matches;
  }

  function reducedMotion() {
    return window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  }

  function reveal(el) {
    var shimmer = stage.querySelector('.share__shimmer');
    if (shimmer) shimmer.remove();
    el.classList.add('is-in');
    live.textContent = word('loaded', { title: state.share ? state.share.title : '' });
  }

  function trouble(key) {
    var shimmer = stage.querySelector('.share__shimmer');
    if (shimmer) shimmer.remove();
    state.trouble = key;
    render();
  }

  function clearStage(kind) {
    Array.prototype.forEach.call(stage.querySelectorAll(':scope > :not(.share__shimmer)'), function (el) { el.remove(); });
    if (!stage.querySelector('.share__shimmer')) {
      var shimmer = document.createElement('div');
      shimmer.className = 'share__shimmer';
      shimmer.setAttribute('aria-hidden', 'true');
      stage.appendChild(shimmer);
    }
    stage.dataset.kind = kind;
  }

  function showImage(url, share) {
    var img = new Image();
    img.className = 'share__image share__content';
    img.alt = share.title;
    img.decoding = 'async';
    img.referrerPolicy = 'no-referrer';
    img.addEventListener('load', function () { reveal(img); });
    img.addEventListener('error', function () { trouble('image'); });
    img.src = url;
    stage.appendChild(img);
  }

  function frame(url, share, sandboxed) {
    var el = document.createElement('iframe');
    el.className = 'share__frame share__content';
    el.title = share.title;
    el.referrerPolicy = 'no-referrer';
    el.setAttribute('allow', '');
    // A shared page is someone's HTML: framed with the sandbox its origin
    // sends too, it has an opaque origin (no cookie, no storage, no form
    // sent, no navigation of this page). A PDF is framed without one: the
    // browsers' own PDF viewers refuse to run in a sandboxed frame, and a
    // PDF runs none of a page's script.
    if (sandboxed) el.setAttribute('sandbox', 'allow-scripts allow-popups');
    el.addEventListener('load', function () { reveal(el); });
    el.src = url;
    stage.appendChild(el);
  }

  var viewerScript = null;

  function modelViewer() {
    if (window.customElements && window.customElements.get('model-viewer')) return Promise.resolve();
    if (viewerScript) return viewerScript;
    viewerScript = new Promise(function (resolve, reject) {
      var script = document.createElement('script');
      script.type = 'module';
      script.src = MODEL_VIEWER;
      script.addEventListener('error', function () { viewerScript = null; reject(new Error('model-viewer')); });
      document.head.appendChild(script);
      window.customElements.whenDefined('model-viewer').then(resolve);
    });
    return viewerScript;
  }

  function showModel(url, share) {
    modelViewer().then(function () {
      var el = document.createElement('model-viewer');
      el.className = 'share__model share__content';
      el.setAttribute('alt', share.title);
      el.setAttribute('camera-controls', '');
      el.setAttribute('touch-action', 'pan-y');
      el.setAttribute('interaction-prompt', 'none');
      el.setAttribute('shadow-intensity', '0.8');
      el.setAttribute('shadow-softness', '0.9');
      el.setAttribute('exposure', '1');
      el.setAttribute('environment-image', 'neutral');
      el.setAttribute('loading', 'eager');
      if (!reducedMotion()) {
        el.setAttribute('auto-rotate', '');
        el.setAttribute('auto-rotate-delay', '1500');
        el.setAttribute('rotation-per-second', '14deg');
      }
      el.addEventListener('load', function () { reveal(el); });
      el.addEventListener('error', function () { trouble('model'); });
      var hint = document.createElement('p');
      hint.className = 'share__hint';
      hint.setAttribute('aria-hidden', 'true');
      hint.textContent = touch() ? word('turnTouch') : word('turn');
      el.addEventListener('camera-change', function (event) {
        if (event.detail && event.detail.source === 'user-interaction') hint.classList.add('is-gone');
      });
      el.setAttribute('src', url);
      stage.appendChild(el);
      stage.appendChild(hint);
    }, function () {
      trouble('viewer');
    });
  }

  function draw(share) {
    var base = contentBase(share);
    if (!base || typeof share.entry !== 'string' || typeof share.title !== 'string') {
      set('error', { fault: { key: 'odd' } });
      return;
    }
    var kind = share.kind;
    var url = fileUrl(base, share.entry);
    state.share = share;
    state.trouble = null;
    clearStage(shownKind(share));
    set('live');
    var canFull = !!(stage.requestFullscreen && document.fullscreenEnabled);
    fullBtn.hidden = !(canFull && (kind === 'page' || kind === 'markdown' || kind === 'model' || kind === 'image'));
    // The file in a tab of its own: for an image or a PDF only (a phone's
    // browser shows no PDF inside a page). A page, an SVG (a document that
    // may run script: the files' origin sends one opened alone back here)
    // or a model is only ever shown here, inside this frame.
    var svg = /\.svg$/i.test(share.entry);
    openLink.hidden = !((kind === 'image' && !svg) || kind === 'pdf');
    if (!openLink.hidden) openLink.href = url;
    else openLink.removeAttribute('href');
    if (kind === 'image') showImage(url, share);
    else if (kind === 'pdf') frame(url, share, false);
    else if (kind === 'page' || kind === 'markdown') frame(url, share, true);
    else if (kind === 'model') showModel(url, share);
  }

  /* ── the share's metadata ──────────────────────────────────── */

  function load() {
    var id = shareId();
    if (id === null) return set('none');
    if (!ID.test(id)) return set('missing');
    set('loading');
    fetch(server() + '/v1/shares/' + encodeURIComponent(id), {
      method: 'GET',
      mode: 'cors',
      credentials: 'omit',
      cache: 'no-store',
      referrerPolicy: 'no-referrer'
    }).then(function (res) {
      return res.text().then(function (body) {
        var json = null;
        try { json = body ? JSON.parse(body) : null; } catch (e) { json = null; }
        return { status: res.status, json: json };
      });
    }).then(function (answer) {
      var json = answer.json;
      if (answer.status === 200 && json && json.id === id) {
        if (KINDS.indexOf(json.kind) === -1) return set('error', { fault: { key: 'newer' } });
        return draw(json);
      }
      if (answer.status === 410) {
        var how = json && json.state === 'taken_down' ? 'taken_down' : json && json.state === 'deleted' ? 'deleted' : 'expired';
        return set('gone', { reason: how });
      }
      if (answer.status === 404 || answer.status === 400) return set('missing');
      if (answer.status === 429) return set('error', { fault: { key: 'busy' } });
      if (answer.status === 200) return set('error', { fault: { key: 'odd' } });
      return set('error', { fault: { key: 'server', fill: { status: String(answer.status) } } });
    }, function () {
      set('error', { fault: { key: 'offline' } });
    });
  }

  /* ── full screen ───────────────────────────────────────────── */

  fullBtn.addEventListener('click', function () {
    if (document.fullscreenElement) document.exitFullscreen();
    else if (stage.requestFullscreen) stage.requestFullscreen().catch(function () { /* refused */ });
  });

  document.getElementById('share-retry').addEventListener('click', load);

  /* ── the report ────────────────────────────────────────────── */

  var dialog = document.getElementById('report-dialog');
  var form = document.getElementById('report-form');
  var send = document.getElementById('report-send');
  var reportTrouble = document.getElementById('report-trouble');
  var thanks = document.getElementById('report-thanks');
  var abuseLink = document.getElementById('report-abuse');
  var abuse = 'abuse@hangarai.dev';

  function step(name) {
    Array.prototype.forEach.call(dialog.querySelectorAll('[data-step]'), function (el) {
      el.hidden = el.dataset.step !== name;
    });
  }

  function reportSays(text) {
    reportTrouble.hidden = !text;
    reportTrouble.textContent = text || '';
  }

  function chosen() {
    var picked = form.querySelector('input[name="reason"]:checked');
    return picked ? picked.value : null;
  }

  form.addEventListener('change', function () {
    send.disabled = !chosen();
  });

  reportBtn.addEventListener('click', function () {
    form.reset();
    send.disabled = true;
    send.removeAttribute('aria-busy');
    reportSays(null);
    step('ask');
    if (state.share && state.share.abuse) {
      abuse = state.share.abuse;
      abuseLink.textContent = abuse;
      abuseLink.href = 'mailto:' + abuse;
    }
    if (typeof dialog.showModal === 'function') dialog.showModal();
    else dialog.setAttribute('open', '');
  });

  document.getElementById('report-cancel').addEventListener('click', function () { dialog.close(); });
  document.getElementById('report-close').addEventListener('click', function () { dialog.close(); });

  form.addEventListener('submit', function (event) {
    event.preventDefault();
    var reason = chosen();
    if (!reason || !state.share || send.getAttribute('aria-busy') === 'true') return;
    var details = String(document.getElementById('report-details').value || '').trim().slice(0, 2000);
    var body = { reason: reason };
    if (details) body.details = details;
    send.setAttribute('aria-busy', 'true');
    reportSays(null);
    fetch(server() + '/v1/shares/' + encodeURIComponent(state.share.id) + '/report', {
      method: 'POST',
      mode: 'cors',
      credentials: 'omit',
      cache: 'no-store',
      referrerPolicy: 'no-referrer',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body)
    }).then(function (res) {
      return res.text().then(function (text) {
        var json = null;
        try { json = text ? JSON.parse(text) : null; } catch (e) { json = null; }
        return { status: res.status, json: json };
      });
    }).then(function (answer) {
      send.removeAttribute('aria-busy');
      if (answer.status >= 200 && answer.status < 300) {
        if (answer.json && typeof answer.json.abuse === 'string') {
          abuse = answer.json.abuse;
          abuseLink.textContent = abuse;
          abuseLink.href = 'mailto:' + abuse;
        }
        thanks.textContent = word('thanks');
        step('done');
        document.getElementById('report-close').focus();
        return;
      }
      if (answer.status === 429) return reportSays(word('reportBusy', { abuse: abuse }));
      if (answer.status === 410) return reportSays(word('reportGone'));
      if (answer.status === 400 && answer.json && answer.json.message) {
        return reportSays(word('reportRefused', { why: answer.json.message }));
      }
      reportSays(word('reportFailed', { abuse: abuse }));
    }, function () {
      send.removeAttribute('aria-busy');
      reportSays(word('reportFailed', { abuse: abuse }));
    });
  });

  /* ── the referral a link carries ───────────────────────────── */

  // A replay's link carries its author's referral code (?ref=…): the
  // foot's "Made with HangarAI" then says "· Try it" and goes to the code's
  // landing, from this page, where the code can be remembered (a link in
  // the shared page itself would open sandboxed, and remember nothing).
  function referral() {
    var code = '';
    try { code = (new URLSearchParams(location.search).get('ref') || '').trim().toLowerCase(); } catch (e) { /* old browser */ }
    return REF.test(code) ? code : null;
  }

  (function () {
    var code = referral();
    var made = document.querySelector('.share-foot__made');
    var tryIt = document.querySelector('.share-foot__try');
    if (!code || !made) return;
    made.href = '/r/' + encodeURIComponent(code);
    if (tryIt) tryIt.hidden = false;
  })();

  /* ── start ─────────────────────────────────────────────────── */

  if (window.HangarLang) window.HangarLang.on(function () { render(); });
  render();
  load();
})();
