/* ═══════════════════════════════════════════════════════════════
   HangarAI buy: hangarai.dev/buy/?product=<name>

   Asks the account server for the product's Stripe checkout and
   replaces itself with it (docs/native/portal.md, "hangarai.dev/buy").
   Takes the license (?license=) or the machine (?machine=) the app
   passes, and the session cookie of a signed-in visitor. Without a
   product it lists them; ?done=<product>&session=cs_… is Stripe's way
   back, where a plan shows the key of the license it made (POST
   /v1/billing/key, asked again for a while until the webhook is in).
   ?mock=1 asks account/mock.js instead, and shows where it would go.
   ═══════════════════════════════════════════════════════════════ */
(function () {
  'use strict';

  var A = window.HangarAccount;
  var root = document.documentElement;
  var params = new URLSearchParams(location.search);
  var MOCK = params.get('mock') === '1';

  var NAMES = {
    pro_monthly: { en: 'HangarAI Pro, monthly · €20 a month', fr: 'HangarAI Pro, mensuel · 20 € par mois' },
    pro_yearly: { en: 'HangarAI Pro, yearly · €192 a year', fr: 'HangarAI Pro, annuel · 192 € par an' },
    max_monthly: { en: 'HangarAI Max, monthly · €39 a month', fr: 'HangarAI Max, mensuel · 39 € par mois' },
    max_yearly: { en: 'HangarAI Max, yearly · €374 a year', fr: 'HangarAI Max, annuel · 374 € par an' },
    credits_1000: { en: '1,000 credits · €3', fr: '1 000 crédits · 3 €' },
    credits_5000: { en: '5,000 credits · €10', fr: '5 000 crédits · 10 €' }
  };

  var product = params.get('product');
  if (A.PRODUCTS.indexOf(product) < 0) product = null;
  var done = params.get('done');
  if (done !== null && A.PRODUCTS.indexOf(done) < 0) done = '';
  // Passed on as they are, or not at all.
  var license = /^[A-Za-z0-9_-]{1,100}$/.test(params.get('license') || '') ? params.get('license') : null;
  var session = /^cs_[A-Za-z0-9_]{1,250}$/.test(params.get('session') || '') ? params.get('session') : null;
  var machine = /^[0-9a-fA-F]{64}$/.test(params.get('machine') || '') ? params.get('machine').toLowerCase() : null;
  if (license) machine = null;

  var api = null;

  function $(id) {
    return document.getElementById(id);
  }

  function show(id, focus) {
    Array.prototype.forEach.call(document.querySelectorAll('.buy__view'), function (view) {
      // (The one <head> showed by data-buy is already on screen.)
      var was = root.dataset.buy ? view.id === 'v-' + root.dataset.buy : !view.hidden;
      view.hidden = view.id !== id;
      // A view that comes in rises; the one that was already there stays.
      view.classList.toggle('enter', view.id === id && !was);
    });
    delete root.dataset.buy;
    var view = $(id);
    var title = view && view.querySelector('.buy__title');
    if (focus && title) title.focus({ preventScroll: true });
    var live = $('live');
    if (live && title) live.textContent = title.textContent.trim();
  }

  // The product's name in both languages; pages.css shows the page's.
  function fillProduct(name) {
    Array.prototype.forEach.call(document.querySelectorAll('[data-slot="product"]'), function (slot) {
      slot.textContent = '';
      if (!name || !NAMES[name]) return;
      ['en', 'fr'].forEach(function (l) {
        var span = document.createElement('span');
        span.dataset.l = l;
        span.textContent = NAMES[name][l];
        slot.appendChild(span);
      });
    });
  }

  function fail(err) {
    var code = err && err.code;
    if (code === 'license_needed') {
      var next = new URL('../account/', location.href);
      next.searchParams.set('buy', product);
      if (MOCK) next.searchParams.set('mock', '1');
      $('needs-signin').href = next.pathname + next.search;
      return show('v-needs', true);
    }
    var known = ['billing_unavailable', 'network', 'already_subscribed', 'rate_limited'];
    var shown = known.indexOf(code) >= 0 ? code : 'other';
    Array.prototype.forEach.call(document.querySelectorAll('[data-error]'), function (p) {
      p.hidden = p.dataset.error !== shown;
    });
    $('error-account').hidden = shown !== 'already_subscribed';
    $('retry').hidden = shown === 'already_subscribed';
    show('v-error', true);
  }

  function go() {
    fillProduct(product);
    show('v-going', false);
    var body = { product: product, 'return': 'buy' };
    if (license) body.license = license;
    if (machine) body.machine = machine;
    api.checkout(body).then(function (res) {
      if (api.mock) {
        $('mock-url').textContent = res.url;
        return show('v-mock', true);
      }
      if (!res || !A.isPaymentUrl(res.url)) return fail({ code: 'billing_unavailable' });
      // Replaced, not pushed: back from Stripe returns where the person
      // came from, not to a page that sends them on again.
      location.replace(res.url);
    }, fail);
  }

  // The key of the license the checkout made. The webhook may come a
  // little after the browser: asked every 1.5 s for 20 s, then the page
  // says HangarAI picks the license up by itself, with a way to ask again.
  // A purchase for a license that has a key already never has one here.
  var KEY_EVERY = 1500;
  var KEY_FOR = 20000;

  function showKey(phase, key) {
    $('key-wait').hidden = phase !== 'wait';
    $('key-got').hidden = phase !== 'got';
    $('key-steps').hidden = phase !== 'got';
    $('key-none').hidden = phase !== 'none';
    $('key-retry').hidden = !session;
    if (key) $('key-value').textContent = key;
  }

  function fetchKey() {
    if (!session) return showKey('none');
    showKey('wait');
    var until = Date.now() + KEY_FOR;
    (function ask() {
      api.billingKey(session).then(function (res) {
        if (res && res.key) return showKey('got', res.key);
        showKey('none');
      }, function (err) {
        var later = err && (err.status === 404 || err.status === 0 || err.status === 429 || err.status >= 500);
        if (later && Date.now() + KEY_EVERY < until) return setTimeout(ask, KEY_EVERY);
        showKey('none');
      });
    })();
  }

  function copyKey() {
    var code = $('key-value');
    var text = code.textContent;
    var button = $('key-copy');
    function copied() {
      var words = button.innerHTML;
      button.classList.add('is-copied');
      button.innerHTML = '<span data-l="en">Copied</span><span data-l="fr">Copié</span>';
      setTimeout(function () {
        button.classList.remove('is-copied');
        button.innerHTML = words;
      }, 1600);
      var live = $('live');
      if (live) live.textContent = root.lang === 'fr' ? 'Clé copiée' : 'Key copied';
    }
    function select() {
      var range = document.createRange();
      range.selectNodeContents(code);
      var sel = window.getSelection();
      sel.removeAllRanges();
      sel.addRange(range);
    }
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(copied, select);
    } else {
      select();
    }
  }

  function start() {
    api = MOCK ? window.HangarMock.create({ as: params.get('as'), slow: params.get('slow') === '1' }) : A.create();
    if (MOCK) {
      $('mock').hidden = false;
      // The list keeps the mock on.
      Array.prototype.forEach.call(document.querySelectorAll('.choose__item'), function (a) {
        a.setAttribute('href', '?mock=1&product=' + a.dataset.product);
      });
    }
    $('retry').addEventListener('click', go);
    $('key-copy').addEventListener('click', copyKey);
    $('key-retry').addEventListener('click', fetchKey);

    if (done !== null) {
      fillProduct(done);
      var credits = /^credits_/.test(done);
      document.querySelector('[data-done="license"]').hidden = credits;
      document.querySelector('[data-done="credits"]').hidden = !credits;
      show('v-done', true);
      if (!credits) fetchKey();
      return;
    }
    if (!product) return show('v-choose', false);
    go();
  }

  function loadMock() {
    return new Promise(function (resolve, reject) {
      var s = document.createElement('script');
      s.src = '../account/mock.js';
      s.onload = resolve;
      s.onerror = reject;
      document.head.appendChild(s);
    });
  }

  (MOCK ? loadMock() : Promise.resolve()).then(start, function () { fail({ code: 'other' }); });
})();
