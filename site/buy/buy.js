/* ═══════════════════════════════════════════════════════════════
   HangarAI buy: hangarai.dev/buy/?product=<name>

   Asks the account server for the product's Polar checkout and
   replaces itself with it (docs/native/portal.md, "hangarai.dev/buy").
   Takes the license (?license=) or the machine (?machine=) the app
   passes, and the session cookie of a signed-in visitor. Without a
   product it lists them; ?done=<product> is Polar's way back.
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
      if (!res || !A.isPolarUrl(res.url)) return fail({ code: 'billing_unavailable' });
      // Replaced, not pushed: back from Polar returns where the person
      // came from, not to a page that sends them on again.
      location.replace(res.url);
    }, fail);
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

    if (done !== null) {
      fillProduct(done);
      var credits = /^credits_/.test(done);
      document.querySelector('[data-done="license"]').hidden = credits;
      document.querySelector('[data-done="credits"]').hidden = !credits;
      return show('v-done', true);
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
