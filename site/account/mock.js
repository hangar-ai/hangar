/* ═══════════════════════════════════════════════════════════════
   HangarAI account: a fake account server, for ?mock=1

   The same methods as assets/api.js, answering inside the page as
   docs/native/portal.md says the server will: the sign-in (the trip
   to GitHub skipped), the summary, the history and its pages,
   deactivating a machine, adding a key, the checkout and the
   customer portal, and their refusals. Nothing leaves the browser.

   &as= picks the account (portal.md, "Mock mode"); &ref= its referral
   program (active, new, starting, waiting, top); &slow=1 makes every
   call take a second; &signin=denied comes back from "GitHub" with
   access_denied. Changes last until the page is reloaded; being
   signed in lasts the tab.
   ═══════════════════════════════════════════════════════════════ */
(function () {
  'use strict';

  var A = window.HangarAccount;
  var DAY = 86400000;
  var SIGNED_IN = 'hangar.mock.session';
  var CHALLENGE = 'hangar.mock.challenge';

  var SCENARIOS = ['pro', 'max', 'monthly', 'canceling', 'lifetime', 'trial', 'free', 'ended', 'none', 'several', 'down'];

  function euro(cents) {
    return { amount: cents, currency: 'EUR' };
  }

  // The person's prices of 2026-11-19 for 3.0 (docs/native/portal.md,
  // "The offers"): Pro and Max, each by the month then by the year.
  function plan(product, tier, interval, cents) {
    var max = tier === 'max';
    return {
      product: product, tier: tier, interval: interval, price: euro(cents),
      credits_per_month: max ? 10000 : 3000, build_machines: max ? 5 : 3, cloud_priority: max
    };
  }
  var PLANS = [
    plan('pro_monthly', 'pro', 'month', 2000),
    plan('pro_yearly', 'pro', 'year', 19200),
    plan('max_monthly', 'max', 'month', 3900),
    plan('max_yearly', 'max', 'year', 37400)
  ];
  var PACKS = [
    { product: 'credits_1000', credits: 1000, price: euro(300) },
    { product: 'credits_5000', credits: 5000, price: euro(1000) }
  ];
  var PAYG_LIMITS = { presets: [500, 1000, 2500, 5000], min: 100, max: 50000 };
  // docs/native/credits-economics.md: a credit carries the same provider
  // cost everywhere, so dictation is 0.66 credit a minute.
  var DICTATION = 0.66;
  var RATES = [{ feature: 'dictation', credits_per_unit: DICTATION, unit: 'minute' }];

  function milli(n) {
    return Math.round(n * 1000) / 1000;
  }

  // What dictation spent, in credits, with its minutes.
  function dictation(credits) {
    return { feature: 'dictation', units: milli(credits / DICTATION), credits: credits };
  }

  function startOfNextMonthUtc(now) {
    var d = new Date(now);
    return Date.UTC(d.getUTCFullYear(), d.getUTCMonth() + 1, 1);
  }

  function payg(now, o) {
    return Object.assign({
      available: true, reason: null, setup: 'ready', enabled: false,
      limit: euro(1000), spent: euro(0), credits: 0,
      price: { amount: 300, currency: 'EUR', credits: 1000 },
      resets_at: startOfNextMonthUtc(now), limits: PAYG_LIMITS
    }, o || {});
  }

  function machine(id, hash, name, os, activatedDaysAgo, seenHoursAgo, now) {
    return {
      id: id, machine: hash, name: name, os: os,
      activated_at: now - activatedDaysAgo * DAY,
      last_seen_at: now - seenHoursAgo * 3600000
    };
  }

  // The accounts &as= names. Dates are counted from now, so that a
  // trial always has its nine days left.
  function build(as, now) {
    var pro = {
      id: 'lic_8Q2TK4MZ', kind: 'subscription', tier: 'paid', plan: 'pro_yearly', status: 'active',
      created_at: now - 200 * DAY, renews_at: now + 165 * DAY, ends_at: null, seats: 3,
      machines: [
        machine('seat_a1', '3f9a1c2e', 'Atelier', 'windows', 200, 2, now),
        machine('seat_a2', '8b04d7e1', null, 'linux', 41, 77, now)
      ],
      credits: {
        allowance: { total: 3000, left: 2219.5, resets_at: now + 11 * DAY },
        purchased_left: 5000,
        spent_this_period: [dictation(780.5)],
        payg: payg(now)
      }
    };
    var licenses;
    var billing = true;
    switch (as) {
      case 'max':
        licenses = [Object.assign({}, pro, {
          id: 'lic_M4XPL4N5', plan: 'max_monthly', renews_at: now + 19 * DAY,
          credits: { allowance: { total: 10000, left: 7342.5, resets_at: now + 19 * DAY }, purchased_left: 0,
            spent_this_period: [dictation(2657.5)],
            payg: payg(now) }
        })];
        break;
      case 'monthly':
        licenses = [Object.assign({}, pro, {
          id: 'lic_3HV7PD2C', plan: 'pro_monthly', status: 'past_due', renews_at: null,
          machines: [pro.machines[0]],
          credits: { allowance: { total: 3000, left: 40, resets_at: now + 3 * DAY }, purchased_left: 0,
            spent_this_period: [dictation(2960)],
            payg: payg(now, { available: false, reason: 'invoice_unpaid' }) }
        })];
        break;
      case 'canceling':
        licenses = [Object.assign({}, pro, {
          id: 'lic_C4NC3L1N', plan: 'pro_monthly', status: 'canceling', renews_at: null, ends_at: now + 6 * DAY,
          credits: { allowance: { total: 3000, left: 0, resets_at: now + 6 * DAY }, purchased_left: 0,
            spent_this_period: [dictation(4400)],
            payg: payg(now, { enabled: true, limit: euro(2500), spent: euro(420), credits: 1400 }) }
        })];
        break;
      case 'lifetime':
        licenses = [{
          id: 'lic_L1F3T1M3', kind: 'lifetime', tier: 'paid', status: 'active',
          created_at: now - 260 * DAY, renews_at: null, ends_at: null, seats: 3,
          machines: [
            machine('seat_b1', '3f9a1c2e', 'Atelier', 'windows', 260, 1, now),
            machine('seat_b2', 'c7e2019a', 'MacBook Air', 'macos', 120, 30, now),
            machine('seat_b3', '51d0ab3f', 'build-box', 'linux', 12, 200, now)
          ],
          credits: { allowance: null, purchased_left: 3612,
            spent_this_period: [dictation(214)],
            payg: payg(now, { setup: 'checkout', limit: null }) }
        }];
        break;
      case 'trial':
        billing = false;
        licenses = [{
          id: 'lic_TR14L5DY', kind: 'trial', tier: 'paid', status: 'active',
          created_at: now - 5 * DAY, renews_at: null, ends_at: now + 9 * DAY, seats: 1,
          machines: [machine('seat_c1', '3f9a1c2e', 'Atelier', 'windows', 5, 0.2, now)],
          credits: { allowance: { total: 150, left: 96, resets_at: now + 9 * DAY }, purchased_left: 0,
            spent_this_period: [dictation(54)],
            payg: payg(now, { available: false, reason: 'first_purchase', limit: null }) }
        }];
        break;
      case 'free':
        billing = false;
        licenses = [{
          id: 'lic_FR33GR4N', kind: 'free_grant', tier: 'free', status: 'active',
          created_at: now - 2 * DAY, renews_at: null, ends_at: null, seats: 1,
          machines: [machine('seat_d1', '8b04d7e1', 'thinkpad', 'linux', 2, 5, now)],
          credits: { allowance: { total: 30, left: 18, resets_at: startOfNextMonthUtc(now) }, purchased_left: 0,
            spent_this_period: [dictation(12)],
            payg: payg(now, { available: false, reason: 'first_purchase', limit: null }) }
        }];
        break;
      case 'ended':
        licenses = [Object.assign({}, pro, {
          id: 'lic_3ND3D0K2', plan: 'pro_monthly', status: 'ended', renews_at: null, ends_at: now - 12 * DAY,
          machines: [pro.machines[0]],
          credits: { allowance: null, purchased_left: 4300, spent_this_period: [],
            payg: payg(now, { available: false, reason: 'ended' }) }
        })];
        break;
      case 'none':
        billing = false;
        licenses = [];
        break;
      case 'several':
        licenses = [pro, {
          id: 'lic_FR33GR4N', kind: 'free_grant', tier: 'free', status: 'active',
          created_at: now - 2 * DAY, renews_at: null, ends_at: null, seats: 1,
          machines: [machine('seat_d1', '51d0ab3f', 'old-laptop', 'linux', 2, 26, now)],
          credits: { allowance: { total: 30, left: 30, resets_at: startOfNextMonthUtc(now) }, purchased_left: 0,
            spent_this_period: [], payg: payg(now, { available: false, reason: 'first_purchase', limit: null }) }
        }];
        break;
      default:
        licenses = [pro];
    }
    return {
      account: { id: 'acc_mock', display_name: 'Ada Lovelace', email: 'ada@example.org', providers: ['github'] },
      billing: { portal: billing },
      licenses: licenses,
      plans: PLANS,
      packs: PACKS,
      rates: RATES
    };
  }

  /* ── the referral program ─────────────────────────────────────
     Made up month by month as the server would count it: each
     customer's payments bring 30 % of their net, held 30 days, then
     payable; on the 1st of each month what is payable goes out if it
     reaches 20 € and a payout profile is there. The statements and
     every figure of GET /v1/referral come out of that one story. */

  var REF_SCENARIOS = ['active', 'new', 'starting', 'waiting', 'top', 'error'];
  var HOLD = 30 * DAY;
  var MINIMUM = 2000;
  var PROGRAM = {
    rate: 30, hold_days: 30, bonus_credits: 1000, attribution_days: 60,
    payout_minimum: euro(MINIMUM),
    // credits-economics.md, "The referral program at 30 %": 30 % of
    // 14.91 €, 147.06 €, 29.50 €, 286.90 €.
    examples: [
      { product: 'pro_monthly', per: 'month', commission: euro(447) },
      { product: 'pro_yearly', per: 'year', commission: euro(4412) },
      { product: 'max_monthly', per: 'month', commission: euro(885) },
      { product: 'max_yearly', per: 'year', commission: euro(8607) }
    ]
  };
  var NET = { pro_monthly: 1491, pro_yearly: 14706, max_monthly: 2950, max_yearly: 28690 };
  var CUT = { pro_monthly: 447, pro_yearly: 4412, max_monthly: 885, max_yearly: 8607 };
  var CODE_RULE = /^[a-z0-9](?:[a-z0-9]|-(?!-)){1,18}[a-z0-9]$/;

  function utcMonthStart(ms, add) {
    var d = new Date(ms);
    return Date.UTC(d.getUTCFullYear(), d.getUTCMonth() + (add || 0), 1);
  }

  function monthKey(ms) {
    return new Date(ms).toISOString().slice(0, 7);
  }

  // A small seeded generator: the same account every load.
  function seeded(seed) {
    var x = seed;
    return function () {
      x = (x * 1103515245 + 12345) % 2147483648;
      return x / 2147483648;
    };
  }

  // shape: how many customers, over how many days, and the rest.
  var REF_SHAPES = {
    'new': { customers: 0, trials: 0, days: 0, profile: false, code: 'q7mk2x9p', visits: 0 },
    starting: { customers: 3, trials: 2, days: 40, profile: false, code: 'ada', visits: 41, mix: ['pro_monthly', 'max_monthly', 'pro_yearly'] },
    waiting: { customers: 9, trials: 3, days: 120, profile: false, code: 'ada', visits: 212 },
    active: { customers: 23, trials: 6, days: 420, profile: true, code: 'ada', visits: 1284, refund: true },
    top: { customers: 128, trials: 14, days: 640, profile: true, code: 'ada', visits: 9871, refund: true }
  };

  function referralOf(as, now) {
    var shape = REF_SHAPES[as] || REF_SHAPES.active;
    var rnd = seeded(as.length * 7919 + shape.customers * 31 + 1);
    var people = [];
    var events = [];
    var products = ['pro_monthly', 'pro_monthly', 'pro_yearly', 'max_monthly', 'pro_monthly', 'pro_yearly', 'max_yearly', 'pro_monthly'];
    var i;
    // Customers come faster as the link travels: more of them lately.
    for (i = 0; i < shape.customers; i++) {
      var f = Math.pow(rnd(), 0.62);
      var since = now - Math.round((1 - f) * shape.days * DAY) - 2 * DAY - Math.round(rnd() * DAY);
      var product = shape.mix ? shape.mix[i % shape.mix.length] : products[Math.floor(rnd() * products.length)];
      var ended = !shape.mix && rnd() < 0.09;
      people.push({ id: 'rf_' + (1000 + i * 37).toString(36).toUpperCase() + String.fromCharCode(65 + i % 26), product: product, since: since, ended: ended });
    }
    people.sort(function (a, b) { return a.since - b.since; });
    people.forEach(function (p, n) {
      var step = /_yearly$/.test(p.product) ? 365 * DAY : 30.44 * DAY;
      var stop = p.ended ? Math.min(now, p.since + (2 + n % 4) * step) : now;
      var count = 0;
      for (var at = p.since; at <= stop; at += step) {
        events.push({ at: at, referral: p.id, product: p.product, kind: 'payment', net: NET[p.product], commission: CUT[p.product] });
        count += 1;
      }
      p.payments = count;
      p.endedAt = p.ended ? stop : null;
    });
    // One customer asked for their money back: the commission is cancelled.
    if (shape.refund && events.length > 8) {
      var r = events[Math.floor(events.length * 0.55)];
      events.push({ at: r.at + 9 * DAY, referral: r.referral, product: r.product, kind: 'refund', net: -r.net, commission: -r.commission, of: r });
      r.cancelled = true;
    }
    events.sort(function (a, b) { return a.at - b.at; });

    // The payouts, on the 1st of each month.
    var payouts = [];
    var paidUpTo = 0;
    if (events.length && shape.profile) {
      for (var day = utcMonthStart(events[0].at, 1); day <= now; day = utcMonthStart(day, 1)) {
        var due = events.filter(function (e) { return !e.paidAt && !e.cancelled && e.kind === 'payment' && e.at + HOLD <= day; });
        var sum = due.reduce(function (t, e) { return t + e.commission; }, 0);
        if (sum >= MINIMUM) {
          due.forEach(function (e) { e.paidAt = day; });
          payouts.push({ at: day, amount: sum, reference: 'HGR-' + monthKey(day) + '-' + String(17 + payouts.length * 3).padStart(4, '0') });
          paidUpTo = day;
        }
      }
    }
    void paidUpTo;
    function statusOf(e) {
      if (e.kind === 'refund' || e.cancelled) return 'cancelled';
      if (e.paidAt) return 'paid';
      return e.at + HOLD <= now ? 'payable' : 'pending';
    }
    var earnings = { earned: 0, pending: 0, payable: 0, paid: 0 };
    var nextRelease = null;
    events.forEach(function (e) {
      var st = statusOf(e);
      if (st === 'cancelled') return;
      earnings[st] += e.commission;
      earnings.earned += e.commission;
      if (st === 'pending') {
        var at = e.at + HOLD;
        if (!nextRelease || at < nextRelease.at - DAY / 2) nextRelease = { at: at, amount: 0 };
        if (Math.abs(at - nextRelease.at) < DAY / 2) nextRelease.amount += e.commission;
      }
    });
    var monthly = 0;
    var referrals = people.map(function (p) {
      var earned = events.filter(function (e) { return e.referral === p.id && statusOf(e) !== 'cancelled'; })
        .reduce(function (t, e) { return t + e.commission; }, 0);
      var refunded = events.some(function (e) { return e.referral === p.id && e.kind === 'refund'; }) && earned === 0;
      var status = refunded ? 'refunded' : p.ended ? 'ended' : 'active';
      var per = status !== 'active' ? null : /_yearly$/.test(p.product) ? Math.round(CUT[p.product] / 12) : CUT[p.product];
      if (per) monthly += per;
      return { id: p.id, product: p.product, status: status, since: p.since, monthly: per === null ? null : euro(per), earned: euro(earned) };
    });
    for (i = 0; i < shape.trials; i++) {
      referrals.push({ id: 'rf_T' + (i * 911 + 4321).toString(36).toUpperCase(), product: null, status: 'trial',
        since: now - Math.round((0.3 + rnd() * 12) * DAY), monthly: null, earned: euro(0) });
    }
    referrals.sort(function (a, b) { return b.since - a.since; });

    // The last twelve weeks, Monday to Sunday UTC.
    var d = new Date(now);
    var monday = Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate()) - ((d.getUTCDay() + 6) % 7) * DAY;
    var weeks = [];
    for (var w = 11; w >= 0; w--) {
      var start = monday - w * 7 * DAY;
      var end = start + 7 * DAY;
      var customers = people.filter(function (p) { return p.since >= start && p.since < end; }).length;
      var trials = referrals.filter(function (x) { return x.status === 'trial' && x.since >= start && x.since < end; }).length + customers;
      var visits = shape.visits ? Math.round(trials * 9 + customers * 3 + rnd() * shape.visits / 60 + (11 - w) * shape.visits / 900) : 0;
      weeks.push({ start: new Date(start).toISOString().slice(0, 10), visits: visits, trials: trials, customers: customers });
    }
    var paying = people.length;
    var milestones = [1, 10, 50, 100].map(function (n) {
      return { customers: n, reached_at: paying >= n ? people[n - 1].since : null };
    });
    var nextDate = utcMonthStart(now, 1);
    var state = earnings.payable <= 0 ? 'none' : earnings.payable < MINIMUM ? 'below_minimum' : shape.profile ? 'scheduled' : 'needs_profile';

    // The statements, one per month with anything in it.
    var months = {};
    events.forEach(function (e) {
      var k = monthKey(e.at);
      (months[k] = months[k] || { lines: [], payout: null }).lines.push(e);
    });
    payouts.forEach(function (p) {
      var k = monthKey(p.at);
      (months[k] = months[k] || { lines: [], payout: null }).payout = p;
    });
    var statements = Object.keys(months).sort().reverse().slice(0, 36).map(function (k) {
      var m = months[k];
      var earned = m.lines.filter(function (e) { return e.kind === 'payment'; }).reduce(function (t, e) { return t + e.commission; }, 0);
      var cancelled = -m.lines.filter(function (e) { return e.kind === 'refund'; }).reduce(function (t, e) { return t + e.commission; }, 0);
      return {
        summary: {
          id: 'st_' + k.replace('-', '_'), month: k, commissions: m.lines.filter(function (e) { return e.kind === 'payment'; }).length,
          earned: euro(earned), cancelled: euro(cancelled),
          payout: m.payout ? { amount: m.payout.amount, currency: 'EUR', paid_at: m.payout.at, method: 'iban', reference: m.payout.reference } : null
        },
        lines: m.lines.map(function (e) {
          return { at: e.at, referral: e.referral, product: e.product, kind: e.kind, net: euro(e.net), commission: euro(e.commission), status: statusOf(e) };
        })
      };
    });

    return {
      data: {
        code: shape.code,
        link: 'https://hangarai.dev/r/' + shape.code,
        code_rules: { min: 3, max: 20 },
        program: PROGRAM,
        stats: { visits: shape.visits, trials: shape.customers + shape.trials + Math.round(shape.customers * 1.9), customers: paying },
        weeks: weeks,
        earnings: {
          earned: euro(earnings.earned), pending: euro(earnings.pending), payable: euro(earnings.payable), paid: euro(earnings.paid),
          monthly: euro(monthly),
          next_release: nextRelease ? { at: nextRelease.at, amount: euro(nextRelease.amount) } : null
        },
        next_payout: { date: new Date(nextDate).toISOString().slice(0, 10), amount: euro(earnings.payable), state: state },
        referrals: referrals.slice(0, 100),
        referrals_more: Math.max(0, referrals.length - 100),
        milestones: milestones,
        payout_profile: shape.profile
          ? { legal_name: 'Ada Lovelace', country: 'FR', method: 'iban', account: 'FR76 •••• •••• •••• •••• •••0 189', terms: '2026-11-19', updated_at: now - 190 * DAY }
          : null,
        referred_by: as === 'new' ? { code: 'lena', name: 'Léna' } : null
      },
      statements: statements
    };
  }

  // ISO 13616: the country's length and the check digits (mod 97).
  var IBAN_LENGTHS = {
    AD: 24, AE: 23, AT: 20, BE: 16, BG: 22, CH: 21, CY: 28, CZ: 24, DE: 22, DK: 18, EE: 20, ES: 24, FI: 18, FR: 27,
    GB: 22, GI: 23, GR: 27, HR: 21, HU: 28, IE: 22, IS: 26, IT: 27, LI: 21, LT: 20, LU: 20, LV: 21, MC: 27, MT: 31,
    NL: 18, NO: 15, PL: 28, PT: 25, RO: 24, SE: 24, SI: 19, SK: 24, SM: 27, VA: 22
  };

  function ibanOk(iban) {
    if (!/^[A-Z]{2}\d{2}[A-Z0-9]{11,30}$/.test(iban)) return false;
    if (IBAN_LENGTHS[iban.slice(0, 2)] && IBAN_LENGTHS[iban.slice(0, 2)] !== iban.length) return false;
    var moved = iban.slice(4) + iban.slice(0, 4);
    var rest = 0;
    for (var i = 0; i < moved.length; i++) {
      var c = moved.charCodeAt(i);
      var v = c >= 65 ? String(c - 55) : moved[i];
      for (var j = 0; j < v.length; j++) rest = (rest * 10 + (+v[j])) % 97;
    }
    return rest === 1;
  }

  function maskAccount(body) {
    if (body.method === 'paypal') {
      var at = body.paypal_email.indexOf('@');
      return body.paypal_email[0] + '•••' + body.paypal_email.slice(at);
    }
    var iban = body.iban;
    var masked = iban.slice(0, 4) + iban.slice(4, -4).replace(/./g, '•') + iban.slice(-4);
    return masked.replace(/(.{4})/g, '$1 ').trim();
  }

  // A license's ledger, newest first: a period's allowance at each
  // start, the packs, one spend per day of dictation.
  function ledger(license, now) {
    var entries = [];
    var seq = 0;
    function add(entry) {
      seq += 1;
      entry.id = 'led_' + license.id.slice(4) + '_' + seq;
      entries.push(entry);
    }
    var created = license.created_at;
    var until = license.status === 'ended' && license.ends_at ? license.ends_at : now;
    var span = Math.min(70, Math.floor((until - created) / DAY));
    var start = until - span * DAY;
    var seed = license.id.charCodeAt(5) || 7;

    if (license.kind === 'trial') add({ at: created, type: 'grant', source: 'trial', credits: 150 });
    if (license.kind === 'lifetime') add({ at: created, type: 'grant', source: 'lifetime', credits: 5000 });
    if (license.kind === 'subscription' || license.kind === 'free_grant') {
      var period = 30;
      var total = license.kind === 'free_grant' ? 30 : /^max_/.test(license.plan || '') ? 10000 : 3000;
      for (var p = Math.max(created, start - period * DAY); p <= until; p += period * DAY) {
        add({ at: p, type: 'grant', source: 'allowance', credits: total });
      }
    }
    if (license.kind !== 'trial' && license.kind !== 'free_grant' && span > 20) {
      add({ at: start + 18 * DAY + 3600000, type: 'grant', source: 'purchase', product: 'credits_5000', credits: 5000 });
      if (license.kind === 'lifetime') {
        add({ at: start + 44 * DAY, type: 'grant', source: 'purchase', product: 'credits_1000', credits: 1000 });
        add({ at: start + 46 * DAY, type: 'removal', source: 'refund', credits: -1000 });
      }
    }
    for (var d = 0; d <= span; d++) {
      var at = start + d * DAY;
      if (at < created) continue;
      var minutes = (d * 7 + seed) % 11;
      if (license.kind === 'free_grant') minutes = d % 4 === 0 ? 3 : 0;
      if (!minutes) continue;
      var day = new Date(at).toISOString().slice(0, 10);
      add({ at: at + 17 * 3600000, type: 'spend', feature: 'dictation', day: day, units: minutes * 3, credits: -milli(minutes * 3 * DICTATION) });
      // What pay as you go carried, on the days past the credits.
      var p = license.credits && license.credits.payg;
      if (p && p.enabled && d > span - 3) {
        add({ at: at + 18 * 3600000, type: 'spend', feature: 'dictation', day: day, payg: true, units: 706.8, credits: -466.5 });
      }
    }
    entries.sort(function (a, b) { return b.at - a.at; });
    return entries;
  }

  function create(options) {
    var as = SCENARIOS.indexOf(options.as) >= 0 ? options.as : 'pro';
    var now = Date.now();
    var data = build(as, now);
    var ledgers = {};
    var claimed = 0;
    var refAs = REF_SCENARIOS.indexOf(options.ref) >= 0 ? options.ref
      : ['trial', 'free', 'none'].indexOf(as) >= 0 ? 'new' : 'active';
    var referral = null;

    function ref() {
      return referral || (referral = referralOf(refAs, now));
    }

    function wait(value, error) {
      var ms = options.slow ? 1000 : 260 + Math.round(Math.random() * 220);
      return new Promise(function (resolve, reject) {
        setTimeout(function () {
          if (as === 'down') return reject(new A.ApiError(0, 'network'));
          if (error) return reject(error);
          resolve(value === undefined ? null : JSON.parse(JSON.stringify(value)));
        }, ms);
      });
    }

    function refuse(status, code) {
      return wait(null, new A.ApiError(status, code));
    }

    function signedIn() {
      try { return sessionStorage.getItem(SIGNED_IN) === '1'; } catch (e) { return false; }
    }

    function find(id) {
      for (var i = 0; i < data.licenses.length; i++) if (data.licenses[i].id === id) return data.licenses[i];
      return null;
    }

    function fakeUrl(kind, product) {
      return 'https://sandbox.polar.sh/mock/' + kind + (product ? '/' + product : '');
    }

    return {
      mock: true,
      scenario: as,
      scenarios: SCENARIOS,
      refScenario: refAs,
      refScenarios: REF_SCENARIOS,

      // GitHub is skipped: the page comes back at once with a code, or
      // with access_denied, and the state it left with.
      startUrl: function (state, challenge) {
        try { sessionStorage.setItem(CHALLENGE, challenge); } catch (e) { /* the exchange then fails, as it should */ }
        var url = new URL(location.href);
        ['code', 'state', 'error'].forEach(function (k) { url.searchParams.delete(k); });
        if (options.signin === 'denied') url.searchParams.set('error', 'access_denied');
        else url.searchParams.set('code', 'mock_' + A.random(12));
        url.searchParams.set('state', state);
        return url.toString();
      },

      session: function (code, verifier) {
        var challenge = null;
        try { challenge = sessionStorage.getItem(CHALLENGE); sessionStorage.removeItem(CHALLENGE); } catch (e) { /* none */ }
        if (!/^mock_/.test(code) || !challenge) return refuse(400, 'invalid_code');
        return A.challengeOf(verifier).then(function (computed) {
          if (computed !== challenge) return refuse(400, 'invalid_code');
          try { sessionStorage.setItem(SIGNED_IN, '1'); } catch (e) { /* the session lasts this page */ }
          return wait({ account: data.account, expires_at: now + 30 * DAY });
        });
      },

      signOut: function () {
        try { sessionStorage.removeItem(SIGNED_IN); } catch (e) { /* nothing kept */ }
        return wait(null);
      },

      summary: function () {
        if (!signedIn()) return refuse(401, 'unauthorized');
        return wait(data);
      },

      history: function (license, before, limit) {
        if (!signedIn()) return refuse(401, 'unauthorized');
        var lic = find(license);
        if (!lic) return refuse(404, 'not_found');
        var all = ledgers[license] || (ledgers[license] = ledger(lic, now));
        var from = before ? parseInt(before.slice(2), 10) || 0 : 0;
        var n = Math.min(Math.max(limit || 30, 1), 100);
        var page = all.slice(from, from + n);
        return wait({ entries: page, next: from + n < all.length ? 'c_' + (from + n) : null });
      },

      deactivate: function (license, seat) {
        if (!signedIn()) return refuse(401, 'unauthorized');
        var lic = find(license);
        var before = lic ? lic.machines.length : 0;
        if (lic) lic.machines = lic.machines.filter(function (m) { return m.id !== seat; });
        if (!lic || lic.machines.length === before) return refuse(404, 'not_found');
        return wait(null);
      },

      // Any well-formed key works but one ending in TAKEN, which is
      // another account's.
      claim: function (key) {
        if (!signedIn()) return refuse(401, 'unauthorized');
        var clean = String(key || '').toUpperCase().replace(/[\s-]/g, '');
        if (!/^HANGAR[0-9A-Z]{25}$/.test(clean)) return refuse(400, 'invalid_license');
        if (/TAKEN$/.test(clean)) return refuse(409, 'license_held');
        claimed += 1;
        var lic = {
          id: 'lic_CLA1M' + claimed, kind: 'subscription', tier: 'paid', plan: 'pro_monthly', status: 'active',
          created_at: now - 30 * DAY, renews_at: now + 20 * DAY, ends_at: null, seats: 3, machines: [],
          credits: { allowance: { total: 3000, left: 3000, resets_at: now + 20 * DAY }, purchased_left: 0, spent_this_period: [],
            payg: payg(now) }
        };
        data.licenses.push(lic);
        return wait({ license: lic });
      },

      setPayg: function (license, body) {
        if (!signedIn()) return refuse(401, 'unauthorized');
        var lic = find(license);
        if (!lic) return refuse(404, 'not_found');
        var p = lic.credits.payg;
        if (!p || !p.available) return refuse(409, 'payg_unavailable');
        if (body.enabled && p.setup === 'checkout' && !p.enabled) return refuse(409, 'payg_checkout_needed');
        if (body.limit) {
          var a = body.limit.amount;
          if (!(a >= p.limits.min && a <= p.limits.max) || a !== Math.round(a)) return refuse(400, 'bad_request');
          p.limit = { amount: a, currency: 'EUR' };
        }
        p.enabled = !!body.enabled;
        return wait({ payg: p });
      },

      checkout: function (body) {
        body = A.withReferral(body);
        var product = body && body.product;
        var tail = body && body.referral_code ? '?referral_code=' + body.referral_code : '';
        if (A.PRODUCTS.indexOf(product) < 0 && product !== 'payg') return refuse(400, 'invalid_product');
        if (product === 'payg') {
          var target = body.license ? find(body.license) : null;
          if (!target) return refuse(400, 'license_needed');
          return wait({ url: fakeUrl('checkout', 'payg') + tail });
        }
        var lic = body.license ? find(body.license) : null;
        if (body.license && signedIn() && !lic) return refuse(404, 'not_found');
        if (!lic && !body.license && !body.machine && signedIn()) lic = data.licenses[0] || null;
        if (/^credits_/.test(product) && !lic && !body.license && !body.machine) return refuse(400, 'license_needed');
        // A plan on a live subscription: Pro to Max is changed in the
        // customer portal, never by a second checkout.
        if (lic && /^(pro|max)_/.test(product) &&
          ((lic.kind === 'subscription' && (lic.status === 'active' || lic.status === 'past_due' || lic.status === 'canceling')) ||
            (lic.kind === 'lifetime' && lic.status === 'active'))) {
          return refuse(409, 'already_subscribed');
        }
        return wait({ url: fakeUrl('checkout', product) + tail });
      },

      billingPortal: function () {
        if (!signedIn()) return refuse(401, 'unauthorized');
        if (!data.billing.portal) return refuse(404, 'no_customer');
        return wait({ url: fakeUrl('portal') });
      },

      /* the referral program ------------------------------------- */

      // &ref=error: the account answers, the referral routes don't.
      referral: function () {
        if (!signedIn()) return refuse(401, 'unauthorized');
        if (refAs === 'error') return refuse(503, 'unavailable');
        return wait(ref().data);
      },

      // `taken` is another account's, `admin` a reserved word.
      setReferralCode: function (code) {
        if (!signedIn()) return refuse(401, 'unauthorized');
        var clean = String(code || '');
        if (!CODE_RULE.test(clean)) return refuse(400, 'code_invalid');
        if (/^(admin|support|hangar|hangarai|polar|root)$/.test(clean)) return refuse(400, 'code_forbidden');
        if (clean === 'taken') return refuse(409, 'code_taken');
        ref().data.code = clean;
        ref().data.link = 'https://hangarai.dev/r/' + clean;
        return wait(ref().data);
      },

      setPayoutProfile: function (body) {
        if (!signedIn()) return refuse(401, 'unauthorized');
        body = body || {};
        var name = String(body.legal_name || '').trim();
        if (name.length < 2 || name.length > 140 || !/^[A-Z]{2}$/.test(body.country || '')) return refuse(400, 'bad_request');
        if (body.terms !== '2026-11-19') return refuse(400, 'terms_needed');
        if (body.method === 'iban') {
          if (!ibanOk(String(body.iban || ''))) return refuse(400, 'iban_invalid');
          if (body.bic && !/^[A-Z0-9]{8}([A-Z0-9]{3})?$/.test(body.bic)) return refuse(400, 'bad_request');
        } else if (body.method === 'paypal') {
          if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(String(body.paypal_email || ''))) return refuse(400, 'paypal_invalid');
        } else {
          return refuse(400, 'bad_request');
        }
        var profile = { legal_name: name, country: body.country, method: body.method, account: maskAccount(body), terms: body.terms, updated_at: Date.now() };
        var r = ref().data;
        r.payout_profile = profile;
        if (r.next_payout.state === 'needs_profile') r.next_payout.state = 'scheduled';
        return wait({ payout_profile: profile });
      },

      statements: function () {
        if (!signedIn()) return refuse(401, 'unauthorized');
        if (refAs === 'error') return refuse(503, 'unavailable');
        return wait({ statements: ref().statements.map(function (s) { return s.summary; }) });
      },

      statement: function (id) {
        if (!signedIn()) return refuse(401, 'unauthorized');
        var found = ref().statements.filter(function (s) { return s.summary.id === id; })[0];
        if (!found) return refuse(404, 'not_found');
        var p = ref().data.payout_profile;
        return wait({ statement: found.summary, lines: found.lines, payee: p ? { legal_name: p.legal_name, country: p.country } : null });
      },

      // Public, for r/: any well-formed code but `nobody`.
      referralVisit: function (code) {
        var clean = String(code || '');
        if (!CODE_RULE.test(clean) || clean === 'nobody') return refuse(404, 'referral_unknown');
        var known = { ada: 'Ada', simon: 'Simon', lena: 'Léna' };
        var first = clean.split('-')[0];
        var name = known[clean] || (/^[a-z]+$/.test(first) && first.length > 2 ? first[0].toUpperCase() + first.slice(1) : null);
        return wait({ code: clean, name: name, bonus_credits: 1000, attribution_days: 60 });
      }
    };
  }

  window.HangarMock = { create: create, SCENARIOS: SCENARIOS };
})();
