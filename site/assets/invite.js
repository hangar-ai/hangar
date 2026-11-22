/* ═══════════════════════════════════════════════════════════════
   HangarAI: the landing of a referral link, hangarai.dev/r/<code>
   (r/index.html; this file is not under r/, which the site's server
   rewrites whole to that page)

   Reads the code from the address (/r/<code>, which the site's server
   rewrites to this page, then ?c=<code>, then #<code>), asks the
   account server whose it is and counts the visit, and remembers it
   for 60 days (assets/api.js: localStorage and the hangar_ref cookie;
   the first invitation wins), so that the checkout carries it. Then
   it says who invited the visitor and what they get. The contract is
   docs/native/portal.md, "The referral program"; ?mock=1 asks
   account/mock.js instead.

   The page is written for a code being read; this only fills in the
   name and the figures (by textContent) and picks the state:
   loading, invited, unknown (a code that is not one) or none.
   ═══════════════════════════════════════════════════════════════ */
(function () {
  'use strict';

  var A = window.HangarAccount;
  var params = new URLSearchParams(location.search);
  var MOCK = params.get('mock') === '1';
  var section = document.querySelector('.invite__in');
  if (!A || !section) return;
  var R = A.referral;

  function $(id) {
    return document.getElementById(id);
  }

  function rawCode() {
    var m = /^\/r\/([^/?#]+)\/?$/.exec(location.pathname);
    if (m) {
      try { return decodeURIComponent(m[1]); } catch (e) { return m[1]; }
    }
    return params.get('c') || (location.hash || '').replace(/^#/, '') || '';
  }

  function fill(slot, text) {
    Array.prototype.forEach.call(document.querySelectorAll('[data-slot="' + slot + '"]'), function (el) {
      el.textContent = text;
    });
  }

  function show(state) {
    section.dataset.state = state;
    Array.prototype.forEach.call(section.querySelectorAll('[data-show]'), function (el) {
      el.hidden = el.dataset.show.split(' ').indexOf(state) < 0;
    });
    if (state === 'loading') return;
    var title = section.querySelector('[data-show~="' + state + '"] .invite__title') ||
      section.querySelector('.invite__state:not([hidden]) .invite__title');
    var live = $('live');
    if (live && title) live.textContent = title.textContent.replace(/\s+/g, ' ').trim();
  }

  function credits(n) {
    fill('credits', new Intl.NumberFormat('en').format(n));
    // A no-break space, as the page's French copy has it (Inter's subset
    // has no glyph for the narrow one Intl gives).
    fill('credits-fr', new Intl.NumberFormat('fr').format(n).replace(/\u202f/g, '\u00a0'));
  }

  // The invitation as this browser now holds it: the code on screen,
  // its days, or the earlier one that stays.
  function held(code) {
    var kept = R.remembered();
    var earlier = kept && kept.code !== code;
    $('invite-earlier').hidden = !earlier;
    $('invite-fine').hidden = !!earlier || !kept;
    fill('kept', earlier ? kept.code : '');
    fill('code', code);
    fill('days', String((kept && kept.days) || R.days));
  }

  function invited(code, res) {
    var name = res && typeof res.name === 'string' && res.name.trim() ? res.name.trim().slice(0, 40) : null;
    fill('name', name || 'A friend');
    fill('name-fr', name || 'Un ami');
    if (name) {
      var initial = $('invite-initial');
      initial.textContent = name.slice(0, 1).toUpperCase();
      initial.classList.add('has-initial');
    }
    if (res && res.bonus_credits) credits(res.bonus_credits);
    show('invited');
    held(code);
  }

  function start(api) {
    var raw = rawCode();
    if (!raw) return show('none');
    var code = R.clean(raw);
    if (!code) return show('unknown');
    fill('code', code);
    api.referralVisit(code).then(function (res) {
      var current = R.clean(res.code) || code;
      // The owner changed their code: remember the new one in its place.
      var kept = R.remembered();
      R.remember(current, !!(kept && kept.code === code && current !== code));
      invited(current, res);
    }, function (err) {
      if (err && err.code === 'referral_unknown') return show('unknown');
      // Unreachable: remember it all the same, the checkout checks it.
      R.remember(code);
      invited(code, null);
    });
  }

  $('forget').addEventListener('click', function () {
    R.forget();
    $('invite-fine').hidden = true;
    $('invite-earlier').hidden = true;
    $('forgotten').hidden = false;
    $('forgotten').setAttribute('tabindex', '-1');
    $('forgotten').focus();
  });

  if (!MOCK) return start(A.create());

  var s = document.createElement('script');
  s.src = '/account/mock.js';
  s.onload = function () {
    var note = $('mock-note');
    note.className = 'mock';
    note.setAttribute('role', 'note');
    note.innerHTML = '<b><span data-l="en">Mock mode</span><span data-l="fr">Mode démo</span></b>' +
      '<span><span data-l="en">Made-up answers, and nothing leaves this page. Try the code nobody.</span>' +
      '<span data-l="fr">Réponses inventées, et rien ne quitte cette page. Essaie le code nobody.</span></span>';
    start(window.HangarMock.create({ as: params.get('as'), slow: params.get('slow') === '1' }));
  };
  s.onerror = function () { start(A.create()); };
  document.head.appendChild(s);
})();
