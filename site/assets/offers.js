/* ═══════════════════════════════════════════════════════════════
   HangarAI site: the figures that follow the server's rates

   A credit is worth the same provider cost everywhere, so what it buys
   moves with the providers' prices (docs/native/credits-economics.md).
   The pages write those figures out as they are today; this asks the
   account server what it charges now (GET /v1/offers, no cookie,
   docs/native/portal.md) and writes them again:

     data-hours="dictation:3000"   hours of dictation 3 000 credits buy
     data-per-credit="dictation"   minutes of dictation one credit buys
     data-rate="image_best"        a feature's rate, in credits a unit

   and, from the referral program's terms (`referral` in the answer,
   for affiliates/ and r/):

     data-ref-cut="pro_monthly"    what one customer of it brings, money;
       data-times="10"             … for ten of them;
       data-per="year"             … over a year (a monthly one × 12)
     data-ref-bonus                the friend's bonus credits

   in the language of the text around each one. When the call fails,
   the page keeps what it was written with.
   ═══════════════════════════════════════════════════════════════ */
(function () {
  'use strict';

  var A = window.HangarAccount;
  var slots = document.querySelectorAll('[data-hours], [data-per-credit], [data-rate], [data-ref-cut], [data-ref-bonus]');
  if (!A || !slots.length || !window.fetch) return;

  function langOf(el) {
    var holder = el.closest('[data-l]');
    return holder ? holder.dataset.l : (document.documentElement.lang || 'en');
  }

  // French groups with a no-break space here, as the pages' copy does
  // (the site's Inter has no glyph for the narrow one Intl gives).
  function fmt(n, lang, digits) {
    return new Intl.NumberFormat(lang, { maximumFractionDigits: digits }).format(n).replace(/\u202f/g, '\u00a0');
  }

  function money(cents, currency, lang) {
    var whole = Math.round(cents) % 100 === 0;
    return new Intl.NumberFormat(lang, {
      style: 'currency', currency: currency || 'EUR',
      minimumFractionDigits: whole ? 0 : 2, maximumFractionDigits: whole ? 0 : 2
    }).format(Math.round(cents) / 100).replace(/\u202f/g, '\u00a0');
  }

  // "About" figures: whole hours, and from 20 on, down to a multiple of 5.
  function approxHours(h) {
    return h >= 20 ? Math.floor(h / 5) * 5 : Math.max(1, Math.round(h));
  }

  fetch(A.server() + '/v1/offers', { mode: 'cors', credentials: 'omit', cache: 'no-store' })
    .then(function (res) { return res.ok ? res.json() : null; })
    .then(function (offers) {
      if (offers && offers.referral) referral(offers.referral);
      if (!offers || !offers.rates) return;
      var rates = {};
      var any = {};
      offers.rates.forEach(function (r) {
        if (!(r.credits_per_unit > 0)) return;
        any[r.feature] = r.credits_per_unit;
        if (r.unit === 'minute') rates[r.feature] = r.credits_per_unit;
      });
      Array.prototype.forEach.call(slots, function (el) {
        var lang = langOf(el);
        if (el.dataset.refCut !== undefined || el.dataset.refBonus !== undefined) return;
        if (el.dataset.hours) {
          var parts = el.dataset.hours.split(':');
          var rate = rates[parts[0]];
          if (rate) el.textContent = fmt(approxHours(+parts[1] / rate / 60), lang, 0);
        } else if (el.dataset.perCredit) {
          var per = rates[el.dataset.perCredit];
          if (per) el.textContent = fmt(1 / per, lang, 1);
        } else if (el.dataset.rate) {
          var each = any[el.dataset.rate];
          if (each) el.textContent = fmt(each, lang, 1);
        }
      });
    })
    .catch(function () { /* the figures as written stay */ });

  function referral(ref) {
    var cuts = {};
    (ref.examples || []).forEach(function (e) {
      if (e.commission && e.commission.amount >= 0) cuts[e.product] = e;
    });
    Array.prototype.forEach.call(slots, function (el) {
      var lang = langOf(el);
      if (el.dataset.refBonus !== undefined) {
        if (ref.bonus_credits) el.textContent = fmt(ref.bonus_credits, lang, 0);
        return;
      }
      var ex = el.dataset.refCut && cuts[el.dataset.refCut];
      if (!ex) return;
      var times = +(el.dataset.times || 1);
      var per = el.dataset.per === 'year' && ex.per === 'month' ? 12 : 1;
      el.textContent = money(ex.commission.amount * times * per, ex.commission.currency, lang);
    });
  }
})();
