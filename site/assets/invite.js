/* ═══════════════════════════════════════════════════════════════
   HangarAI: the landing of a referral link, hangarai.dev/r/<code>
   (r/index.html; this file is not under r/, which the site's server
   rewrites whole to that page)

   Reads the code from the address (/r/<code>, which the site's server
   rewrites to this page, then ?c=<code>, then #<code>), asks the
   account server whose it is and counts the visit (on the server,
   nothing in the browser), then says who invited the visitor and what
   they get, and asks « Keep this invitation? ». Only "Accept" keeps the
   code, for 60 days (assets/api.js: localStorage and the hangar_ref
   cookie; the first invitation wins), so that the checkout carries it:
   the CNIL counts a tracker of affiliation among those that need
   consent. "Refuse" keeps nothing, and the page goes on as it is. The
   contract is docs/native/portal.md, "The referral program"; ?mock=1
   asks account/mock.js instead.

   The page is written for a code being read; this only fills in the
   name and the figures (by textContent) and picks the state:
   loading, invited, unknown (a code that is not one) or none. /r/
   without a code shows the invitation this browser holds, if any, with
   « Remove the invitation », which privacy.html points to.
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

  // The code of the link on screen, once the server said whose it is:
  // what "Accept" keeps. Null on /r/ without one, or a code that is none.
  var offered = null;

  // The invitation as this browser now holds it: the code on screen and
  // its days, the earlier one that stays, or none yet, and then the
  // question (only with a code on screen to keep).
  function held() {
    var kept = R.remembered();
    var earlier = !!(kept && offered && kept.code !== offered);
    $('invite-earlier').hidden = !earlier;
    $('invite-fine').hidden = earlier || !kept;
    $('invite-ask').hidden = !!kept || !offered;
    fill('kept', earlier ? kept.code : '');
    fill('held', kept ? kept.code : '');
    fill('days', String((kept && kept.days) || R.days));
  }

  // A message in place of the question or of the held invitation, read
  // out and focused like the rest of the page's answers.
  function answer(id) {
    ['refused', 'forgotten'].forEach(function (other) { $(other).hidden = other !== id; });
    var el = $(id);
    el.setAttribute('tabindex', '-1');
    el.focus();
  }

  function invited(code, res) {
    offered = code;
    var name = res && typeof res.name === 'string' && res.name.trim() ? res.name.trim().slice(0, 40) : null;
    fill('name', name || 'A friend');
    fill('name-fr', name || 'Un ami');
    if (name) {
      var initial = $('invite-initial');
      initial.textContent = name.slice(0, 1).toUpperCase();
      initial.classList.add('has-initial');
    }
    if (res && res.bonus_credits) credits(res.bonus_credits);
    fill('code', code);
    show('invited');
    held();
  }

  function start(api) {
    var raw = rawCode();
    if (!raw) {
      show('none');
      return held();
    }
    var code = R.clean(raw);
    if (!code) return show('unknown');
    fill('code', code);
    api.referralVisit(code).then(function (res) {
      var current = R.clean(res.code) || code;
      // The owner changed their code: the invitation this browser was
      // given leave to keep takes the new one in its place. Nothing new
      // is kept without "Accept".
      var kept = R.remembered();
      if (kept && kept.code === code && current !== code) R.remember(current, true);
      invited(current, res);
    }, function (err) {
      if (err && err.code === 'referral_unknown') return show('unknown');
      // Unreachable: the question all the same, the checkout checks it.
      invited(code, null);
    });
  }

  $('consent-yes').addEventListener('click', function () {
    if (!offered) return;
    R.remember(offered);
    $('refused').hidden = true;
    $('forgotten').hidden = true;
    held();
    var fine = $('invite-fine');
    fine.setAttribute('tabindex', '-1');
    fine.focus();
  });

  $('consent-no').addEventListener('click', function () {
    $('invite-ask').hidden = true;
    answer('refused');
  });

  function forget() {
    R.forget();
    $('invite-fine').hidden = true;
    $('invite-earlier').hidden = true;
    answer('forgotten');
    // On a link's page, the question again: its invitation may be kept now.
    if (offered) $('invite-ask').hidden = false;
  }

  $('forget').addEventListener('click', forget);
  $('forget-earlier').addEventListener('click', forget);

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
