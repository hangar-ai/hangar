/* ═══════════════════════════════════════════════════════════════
   HangarAI site: the account server, as account/ and buy/ call it

   The contract is docs/native/portal.md. Every call is a fetch to
   https://account.hangarai.dev with the session cookie
   (credentials: "include"), JSON in and out, Content-Type the only
   header. A refusal comes back as an ApiError with the protocol's
   code (`unauthorized`, `license_needed`…), a network failure as the
   code `network`. account/mock.js makes an object with the same
   methods for ?mock=1.

   It also keeps the referral code a visitor came with, once they said
   yes on r/ (60 days, the first one winning), and sends it with every
   checkout of the site as `referral_code` (portal.md, "The referral
   program"). Without that yes nothing is kept and nothing is sent.
   ═══════════════════════════════════════════════════════════════ */
(function () {
  'use strict';

  var SERVER = 'https://account.hangarai.dev';
  var API_KEY = 'hangar.api';
  // What buy/ and the account sell. Pay as you go's checkout (`payg`, a
  // card saved on a 0 € subscription) is started from the account only.
  var PRODUCTS = ['pro_monthly', 'pro_yearly', 'max_monthly', 'max_yearly', 'credits_1000', 'credits_5000'];

  /* ── the referral code a visitor came with ─────────────────────
     In localStorage ({code, at, consent}) for the pages, and in a
     cookie on hangarai.dev for the account server, which may read it
     at the checkout when the body has none. Only after the visitor's
     "Accept" on r/: the CNIL counts a tracker of affiliation among
     those that need consent. 60 days; the first code wins. */

  var REF_KEY = 'hangar.ref';
  var REF_COOKIE = 'hangar_ref';
  var REF_DAYS = 60;
  var REF_CODE = /^[a-z0-9](?:[a-z0-9]|-(?!-)){1,18}[a-z0-9]$/;

  function cleanCode(raw) {
    var code = String(raw || '').trim().toLowerCase();
    return REF_CODE.test(code) ? code : null;
  }

  function readCookie() {
    var m = /(?:^|;\s*)hangar_ref=([a-z0-9-]{3,20})(?:;|$)/.exec(document.cookie || '');
    return m ? m[1] : null;
  }

  function writeCookie(code, days) {
    var parts = [REF_COOKIE + '=' + (code || ''), 'Path=/', 'Max-Age=' + Math.round(days * 86400), 'SameSite=Lax'];
    if (location.protocol === 'https:') parts.push('Secure');
    // On hangarai.dev the cookie is the account server's to read too;
    // anywhere else (a loopback address) it stays on the page's host.
    if (/(^|\.)hangarai\.dev$/.test(location.hostname)) parts.push('Domain=hangarai.dev');
    try { document.cookie = parts.join('; '); } catch (e) { /* cookies refused */ }
  }

  // The code remembered now, or null: {code, at, consent, days} with the
  // days left. Only a code kept with the visitor's yes on r/ counts
  // (`consent`, when it was given): one kept before r/ asked is removed.
  function remembered() {
    var raw = null;
    var storage = true;
    try { raw = localStorage.getItem(REF_KEY); } catch (e) { storage = false; }
    var kept = null;
    try { kept = JSON.parse(raw || 'null'); } catch (e) { kept = null; }
    if (kept && typeof kept.consent !== 'number') {
      forget();
      return null;
    }
    var now = Date.now();
    if (kept && cleanCode(kept.code) && typeof kept.at === 'number' && now - kept.at < REF_DAYS * 86400000 && kept.at <= now + 60000) {
      return { code: kept.code, at: kept.at, consent: kept.consent, days: Math.max(1, Math.ceil((kept.at + REF_DAYS * 86400000 - now) / 86400000)) };
    }
    if (kept) {
      // Past its days (or not a code): it goes.
      forget();
      return null;
    }
    var cookie = readCookie();
    if (!cookie) return null;
    // remember() writes both: a cookie alone, where the storage works, is
    // one kept before r/ asked (or past its days).
    if (storage) {
      writeCookie('', 0);
      return null;
    }
    // A browser that keeps cookies but not storage still has the cookie.
    return { code: cookie, at: null, consent: null, days: null };
  }

  // Keeps `code`, once the visitor said yes on r/ (and only then: it is
  // a tracker of affiliation), unless another one is still remembered
  // (the first one wins), or always with `replace` (the server said the
  // code of the one kept changed).
  function remember(raw, replace) {
    var code = cleanCode(raw);
    if (!code) return remembered();
    var kept = remembered();
    if (kept && kept.code !== code && !replace) return kept;
    var at = kept && kept.at && kept.code === code ? kept.at : replace && kept && kept.at ? kept.at : Date.now();
    var consent = (kept && kept.consent) || Date.now();
    try { localStorage.setItem(REF_KEY, JSON.stringify({ code: code, at: at, consent: consent })); } catch (e) { /* the cookie still has it */ }
    writeCookie(code, Math.max(1 / 24, (at + REF_DAYS * 86400000 - Date.now()) / 86400000));
    return remembered();
  }

  function forget() {
    try { localStorage.removeItem(REF_KEY); } catch (e) { /* nothing kept */ }
    writeCookie('', 0);
  }

  function ApiError(status, code, retryAfter) {
    this.status = status;
    this.code = code || 'unknown';
    this.retryAfter = retryAfter || null;
  }

  var LOOPBACK = /^(127\.0\.0\.1|localhost|\[::1\])$/;

  // A page served from a loopback address may name a local server
  // (?api=http://127.0.0.1:<port>, kept for the tab so that the sign-in's
  // return finds it); anywhere else it is always the real one.
  function server() {
    if (!LOOPBACK.test(location.hostname)) return SERVER;
    var asked = null;
    try { asked = new URLSearchParams(location.search).get('api'); } catch (e) { /* old browser */ }
    var ok = /^http:\/\/(127\.0\.0\.1|localhost|\[::1\]):\d{2,5}$/;
    try {
      if (asked && ok.test(asked)) sessionStorage.setItem(API_KEY, asked);
      var kept = sessionStorage.getItem(API_KEY);
      if (kept && ok.test(kept)) return kept;
    } catch (e) { /* storage refused */ }
    return SERVER;
  }

  function create() {
    var base = server();

    function call(method, path, body, anonymous) {
      var init = { method: method, mode: 'cors', credentials: anonymous ? 'omit' : 'include', cache: 'no-store', headers: {} };
      if (body !== undefined) {
        init.headers['Content-Type'] = 'application/json';
        init.body = JSON.stringify(body);
      }
      return fetch(base + path, init).then(function (res) {
        if (res.status === 204) return null;
        return res.json().then(null, function () { return null; }).then(function (data) {
          if (res.ok && data) return data;
          throw new ApiError(res.status, data && data.error, res.headers.get('Retry-After'));
        });
      }, function () {
        throw new ApiError(0, 'network');
      });
    }

    function q(value) {
      return encodeURIComponent(value);
    }

    return {
      mock: false,
      server: base,
      startUrl: function (state, challenge) {
        return base + '/v1/auth/start?provider=github&client=web&state=' + q(state) + '&challenge=' + q(challenge);
      },
      session: function (code, verifier) { return call('POST', '/v1/portal/session', { code: code, verifier: verifier }); },
      signOut: function () { return call('DELETE', '/v1/portal/session'); },
      summary: function () { return call('GET', '/v1/portal/summary'); },
      history: function (license, before, limit) {
        var path = '/v1/portal/licenses/' + q(license) + '/credits/history?limit=' + (limit || 30);
        if (before) path += '&before=' + q(before);
        return call('GET', path);
      },
      deactivate: function (license, seat) {
        return call('DELETE', '/v1/portal/licenses/' + q(license) + '/machines/' + q(seat));
      },
      claim: function (key) { return call('POST', '/v1/portal/licenses', { key: key }); },
      setPayg: function (license, body) { return call('PUT', '/v1/portal/licenses/' + q(license) + '/payg', body); },
      checkout: function (body) { return call('POST', '/v1/billing/checkout', withReferral(body)); },
      billingPortal: function (body) { return call('POST', '/v1/billing/portal', body || {}); },
      // The key of the license a checkout made, from its Checkout Session
      // (`cs_…`, on the way back); 404 until the payment is in, or when it
      // went on a license that had one already.
      billingKey: function (session) { return call('POST', '/v1/billing/key', { session: session }, true); },
      // A new key for a license of the account: the old one stops working,
      // the computers already active keep it.
      newKey: function (license) { return call('POST', '/v1/portal/licenses/' + q(license) + '/key'); },

      // The referral program (portal.md, "The referral program").
      referral: function () { return call('GET', '/v1/referral'); },
      setReferralCode: function (code) { return call('PUT', '/v1/referral/code', { code: code }); },
      setPayoutProfile: function (body) { return call('PUT', '/v1/referral/payout-profile', body); },
      statements: function () { return call('GET', '/v1/referral/statements'); },
      statement: function (id) { return call('GET', '/v1/referral/statements/' + q(id)); },
      referralVisit: function (code) { return call('POST', '/v1/referral/visits', { code: code }, true); }
    };
  }

  // Every checkout of the site carries the code the visitor came with,
  // unless the caller named one.
  function withReferral(body) {
    var kept = remembered();
    if (!kept || (body && body.referral_code)) return body;
    return Object.assign({}, body, { referral_code: kept.code });
  }

  /* ── PKCE and state for the web sign-in (RFC 7636) ───────────── */

  function base64url(bytes) {
    var text = '';
    for (var i = 0; i < bytes.length; i++) text += String.fromCharCode(bytes[i]);
    return btoa(text).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
  }

  function random(n) {
    var bytes = new Uint8Array(n);
    crypto.getRandomValues(bytes);
    return base64url(bytes);
  }

  function challengeOf(verifier) {
    return crypto.subtle.digest('SHA-256', new TextEncoder().encode(verifier)).then(function (hash) {
      return base64url(new Uint8Array(hash));
    });
  }

  // 32 random bytes: a 43-character verifier, and a state of 22.
  function pkce() {
    var verifier = random(32);
    return challengeOf(verifier).then(function (challenge) {
      return { state: random(16), verifier: verifier, challenge: challenge };
    });
  }

  // Where a checkout or the customer portal may send the browser: Stripe's
  // Checkout and customer portal (those exact hosts: other subdomains of
  // stripe.com serve what accounts upload), or Polar for a subscription
  // bought there before Stripe, over HTTPS, and nowhere else.
  var STRIPE_HOSTS = ['checkout.stripe.com', 'billing.stripe.com'];
  function isPaymentUrl(url) {
    try {
      var u = new URL(url);
      if (u.protocol !== 'https:' || u.username || u.password || u.port) return false;
      var host = u.hostname;
      return STRIPE_HOSTS.indexOf(host) >= 0 || host === 'polar.sh' || /\.polar\.sh$/.test(host);
    } catch (e) {
      return false;
    }
  }

  window.HangarAccount = {
    create: create,
    server: server,
    ApiError: ApiError,
    PRODUCTS: PRODUCTS,
    pkce: pkce,
    challengeOf: challengeOf,
    random: random,
    isPaymentUrl: isPaymentUrl,
    withReferral: withReferral,
    referral: { clean: cleanCode, remembered: remembered, remember: remember, forget: forget, days: REF_DAYS }
  };
})();
