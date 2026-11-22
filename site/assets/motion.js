/* ═══════════════════════════════════════════════════════════════
   HangarAI site: the motion

   Scroll-driven, with GSAP and ScrollTrigger (vendor/, 3.15.0) and
   nothing else. The page is written in its final state and reads
   whole without this file: everything here starts from that state,
   moves only transform and opacity, and gives it back on exit.

     full    960 px and wider, motion allowed: tilts, pins, scrubs
     light   narrower, motion allowed: the same story, no pin, no tilt
     reduce  prefers-reduced-motion: plain fades, nothing travels

   index.html adds `motion` to <html> before the first paint when
   motion is allowed, and takes it back after 3 s if this file never
   ran; styles.css only hides the hero's copy under it (until
   `motion-ready`), so a page without script shows everything.

   Every number worth tuning is in TUNE below.
   ═══════════════════════════════════════════════════════════════ */
(function () {
  'use strict';

  var root = document.documentElement;
  var gsap = window.gsap;
  var ScrollTrigger = window.ScrollTrigger;
  if (!gsap || !ScrollTrigger) {
    root.classList.remove('motion');
    return;
  }
  gsap.registerPlugin(ScrollTrigger);

  var TUNE = {
    scrub: 0.6,            // seconds a scrubbed tween takes to catch up with the scroll
    heroTilt: 18,          // deg the hero window leans back at the top of the page
    heroScale: 0.88,       // and how small it starts
    heroLift: 60,          // px it rises by
    heroFlatAt: 'top 14%', // where its top is once it lies flat
    glowDrift: 28,         // % of its height the glow drifts down across the hero
    tilt: 10,              // deg the other windows lean back as they come in
    reveal: 36,            // px a card rises as it fades in (half on a phone)
    lineStagger: 0.09,     // s between two lines of a heading
    countFor: 1.1,         // s a figure takes to count up
    headlineFor: 4.5,      // s each of the hero's headlines stays
    headlineBlur: 8,       // px of blur a headline's words come in from and go out to
    headlineRise: 10       // px they come up by (and go up by on the way out)
  };

  var Q = function (sel, ctx) {
    return Array.prototype.slice.call((ctx || document).querySelectorAll(sel));
  };
  var allowed = window.matchMedia('(prefers-reduced-motion: no-preference)').matches;
  // The site's bar, which sticks at the top: what pins stands under it.
  var barHeight = function () {
    var bar = document.getElementById('bar');
    return bar ? bar.offsetHeight : 0;
  };

  /* ── the hero's entrance ───────────────────────────────────────
     Once, at load, timed rather than scrolled: the headline's words
     rise in turn, the rest follows. styles.css held the copy hidden
     until now, so nothing shows before it moves. Then the headlines
     start taking turns (done). */

  function heroIntro(done) {
    var copy = document.querySelector('.hero__copy');
    var heads = copy && copy.querySelector('.hero__heads');
    var title = heads ? heads.querySelector('.rot__item.is-on') : copy && copy.querySelector('.hero__title');
    if (!copy || !title) return done();
    var line = lineOf(title);
    var html = line.innerHTML;
    if (line !== title) title.setAttribute('aria-label', line.textContent);
    var words = splitWords(line);
    var rest = Q('.hero__copy > *').filter(function (el) { return el !== title && !el.contains(title); });
    gsap.timeline({
      defaults: { ease: 'power3.out' },
      onComplete: function () {
        line.innerHTML = html;
        line.removeAttribute('aria-label');
        title.removeAttribute('aria-label');
        gsap.set(rest, { clearProps: 'transform,opacity' });
        done();
      }
    })
      .from(rest[0], { y: 10, opacity: 0, duration: 0.6 }, 0)
      .from(words, { yPercent: 55, opacity: 0, duration: 0.9, stagger: 0.055 }, 0.08)
      .from(rest.slice(1), { y: 16, opacity: 0, duration: 0.7, stagger: 0.07 }, 0.4);
  }

  // The text of an element in the language on screen.
  function lineOf(el) {
    return el.querySelector('[data-l="' + (root.dataset.lang === 'fr' ? 'fr' : 'en') + '"]') || el;
  }

  /* ── the hero's three headlines take turns ──────────────────────
     Each stays TUNE.headlineFor, its pill under the sentence filling
     meanwhile; then the headline and its sentence leave (fading, a
     little blurred, a few pixels up) as the next ones come in the
     same way, word by word. A pill goes to its headline. They wait
     while the pointer is over them, while a pill has the keyboard's
     focus, while the tab is hidden and while the hero is off screen.
     With reduced motion they do not turn by themselves: a pill
     changes the headline with a slow fade. The cells are as tall as
     their tallest headline (styles.css, .rot), so nothing moves. A
     change of language keeps the headline on screen. */

  function heroTurns() {
    var heads = document.querySelector('.hero__heads');
    var ledes = document.querySelector('.hero__ledes');
    var dots = document.querySelector('.rot-dots');
    if (!heads || !ledes || !dots) return function () {};
    var sets = [Q('.rot__item', heads), Q('.rot__item', ledes)];
    var pills = Q('.rot-dot', dots);
    var n = pills.length;
    if (n < 2 || sets[0].length !== n || sets[1].length !== n) return function () {};

    var calm = !allowed;
    var at = 0;
    var fill = null;    // the pill filling: when it is full, the next headline
    var swap = null;    // the change under way
    var split = [];     // what a change cut into words, to give back
    var holds = {};     // why the headlines wait

    root.classList.add('rot-on');

    function mark() {
      pills.forEach(function (pill, i) {
        pill.setAttribute('aria-current', String(i === at));
        gsap.set(pill.querySelector('b'), { scaleX: calm && i === at ? 1 : 0 });
      });
    }

    function held() {
      return Object.keys(holds).some(function (k) { return holds[k]; });
    }

    function hold(why, on) {
      holds[why] = on;
      if (!fill) return;
      if (held()) fill.pause();
      else fill.resume();
    }

    function wait() {
      if (fill) fill.kill();
      fill = null;
      mark();
      if (calm) return;
      fill = gsap.to(pills[at].querySelector('b'), {
        scaleX: 1,
        duration: TUNE.headlineFor,
        ease: 'none',
        onComplete: function () { go((at + 1) % n); }
      });
      if (held()) fill.pause();
    }

    // Whatever a change left half done, finished: the headline `at` on
    // screen, the others away, every text whole again.
    function settle() {
      if (swap) swap.kill();
      swap = null;
      split.forEach(function (s) {
        s.line.innerHTML = s.html;
        s.line.removeAttribute('aria-label');
        if (s.label) s.item.removeAttribute('aria-label');
      });
      split = [];
      sets.forEach(function (items) {
        items.forEach(function (item, i) { item.classList.toggle('is-on', i === at); });
        gsap.set(items, { clearProps: 'opacity,filter,transform' });
      });
    }

    function go(to) {
      if (to === at) return;
      settle();
      var from = at;
      at = to;
      swap = gsap.timeline({ onComplete: settle });
      sets.forEach(function (items, k) {
        var out = items[from];
        var inn = items[to];
        var delay = k * 0.08;
        if (calm) {
          swap.to(out, { opacity: 0, duration: 0.6, ease: 'none' }, delay);
          swap.fromTo(inn, { opacity: 0 }, { opacity: 1, duration: 0.9, ease: 'none', immediateRender: true }, delay + 0.3);
          inn.classList.add('is-on');
          return;
        }
        var line = lineOf(inn);
        var label = !inn.hasAttribute('aria-hidden');
        split.push({ item: inn, line: line, html: line.innerHTML, label: label });
        if (label) inn.setAttribute('aria-label', line.textContent);
        var words = splitWords(line);
        swap.to(out, {
          opacity: 0,
          filter: 'blur(' + TUNE.headlineBlur * 0.75 + 'px)',
          y: -TUNE.headlineRise * 0.8,
          duration: 0.45,
          ease: 'power2.in'
        }, delay);
        swap.fromTo(words, {
          opacity: 0,
          filter: 'blur(' + TUNE.headlineBlur + 'px)',
          y: TUNE.headlineRise
        }, {
          opacity: 1,
          filter: 'blur(0px)',
          y: 0,
          duration: 0.7,
          ease: 'power3.out',
          stagger: k ? 0.012 : 0.05,
          immediateRender: true
        }, delay + 0.28);
        inn.classList.add('is-on');
      });
      wait();
    }

    pills.forEach(function (pill, i) {
      pill.addEventListener('click', function () {
        if (i === at) return;
        go(i);
      });
    });
    [heads, ledes, dots].forEach(function (el, i) {
      el.addEventListener('pointerenter', function (e) { if (e.pointerType === 'mouse') hold('over' + i, true); });
      el.addEventListener('pointerleave', function () { hold('over' + i, false); });
    });
    dots.addEventListener('focusin', function (e) {
      hold('focus', !!(e.target.matches && e.target.matches(':focus-visible')));
    });
    dots.addEventListener('focusout', function (e) {
      if (!dots.contains(e.relatedTarget)) hold('focus', false);
    });
    document.addEventListener('visibilitychange', function () { hold('tab', document.hidden); });
    if ('IntersectionObserver' in window) {
      new IntersectionObserver(function (entries) {
        hold('away', !entries[entries.length - 1].isIntersecting);
      }).observe(heads);
    }
    // Another language: the same headline, whole, in it.
    if (window.HangarLang) window.HangarLang.on(settle);

    mark();
    return wait;
  }

  // Each word in a span of its own; the &nbsp; pairs stay together.
  function splitWords(el) {
    el.setAttribute('aria-label', el.textContent);
    var parts = el.textContent.split(' ');
    el.textContent = '';
    return parts.map(function (word, i) {
      if (i) el.appendChild(document.createTextNode(' '));
      var span = document.createElement('span');
      span.className = 'w';
      span.setAttribute('aria-hidden', 'true');
      span.textContent = word;
      el.appendChild(span);
      return span;
    });
  }

  /* ── headings: masked line reveals ─────────────────────────────
     A title is cut into its lines when it comes into view (so the
     lines are the ones the visitor's width and font give), each line
     rises out of its own mask, and the title gets its text back. */

  function headings(conditions, cleanups) {
    Q('.sec__title').forEach(function (title) {
      // A title in two languages (pricing/): the one on screen rises.
      var el = title.querySelector('[data-l]')
        ? title.querySelector('[data-l="' + (root.dataset.lang === 'fr' ? 'fr' : 'en') + '"]')
        : title;
      if (!el || ScrollTrigger.isInViewport(el, 0.1) || el.children.length) return;
      var html = el.innerHTML;
      gsap.set(el, { opacity: 0 });
      ScrollTrigger.create({
        trigger: el,
        start: 'top 88%',
        once: true,
        onEnter: function () {
          if (conditions.reduce) {
            gsap.to(el, { opacity: 1, duration: 0.6, ease: 'none' });
            return;
          }
          var lines = splitLines(el);
          gsap.set(el, { opacity: 1 });
          gsap.from(lines, {
            yPercent: 105,
            duration: 0.95,
            ease: 'power4.out',
            stagger: TUNE.lineStagger,
            onComplete: function () { restore(); }
          });
        }
      });
      function restore() {
        el.innerHTML = html;
        el.removeAttribute('aria-label');
      }
      cleanups.push(restore);
    });
  }

  function splitLines(el) {
    var text = el.textContent;
    el.setAttribute('aria-label', text);
    var words = splitWords(el);
    // One read pass, then one write pass.
    var tops = words.map(function (w) { return w.offsetTop; });
    var lines = [];
    words.forEach(function (w, i) {
      if (!i || tops[i] - tops[i - 1] > 2) lines.push([]);
      lines[lines.length - 1].push(w.textContent);
    });
    el.textContent = '';
    return lines.map(function (line) {
      var mask = document.createElement('span');
      mask.className = 'rl';
      mask.setAttribute('aria-hidden', 'true');
      var inner = document.createElement('span');
      inner.textContent = line.join(' ');
      mask.appendChild(inner);
      el.appendChild(mask);
      return inner;
    });
  }

  /* ── reveals: cards and short texts fade up as they come in ──── */

  var REVEAL = [
    '.head .sec__lede', '.head .eyebrow', '.pains > .pain', '.steps > .step', '.film',
    '.pillar__copy > .eyebrow', '.pillar__copy > .sec__lede', '.pillar__copy .points li',
    '.pillar__copy > .fine', '.pillar__art:not(.pillar__art--phones)', '.facts .fold',
    '.phones__note', '.reach__card', '.themes', '.native', '.specs .fold',
    '.period', '.plans > .plan', '.credits-line', '.cred__card', '.plat', '.after', '.qa details',
    '.aff-reveal', '.aff-fact'
  ];

  function reveals(conditions, extra) {
    var rise = conditions.reduce ? 0 : conditions.full ? TUNE.reveal : TUNE.reveal / 2;
    var els = Q(REVEAL.join(',')).concat(extra).filter(function (el) {
      return !ScrollTrigger.isInViewport(el, 0.05);
    });
    if (!els.length) return;
    gsap.set(els, { opacity: 0, y: rise });
    ScrollTrigger.batch(els, {
      start: 'top 90%',
      once: true,
      onEnter: function (batch) {
        gsap.to(batch, {
          opacity: 1,
          y: 0,
          duration: conditions.reduce ? 0.5 : 0.8,
          ease: conditions.reduce ? 'none' : 'power3.out',
          stagger: 0.08,
          overwrite: true
        });
      }
    });
  }

  /* ── the hero: the window leans back, then lies flat ─────────── */

  function heroScroll(conditions) {
    var win = document.querySelector('.shot--hero .window');
    if (win) {
      var from = conditions.full
        ? { transformPerspective: 1400, y: TUNE.heroLift, rotateX: TUNE.heroTilt, scale: TUNE.heroScale }
        : { y: TUNE.heroLift / 2, scale: 0.96 };
      gsap.fromTo(win, from, {
        y: 0, rotateX: 0, scale: 1, ease: 'none',
        scrollTrigger: { trigger: win, start: 0, end: TUNE.heroFlatAt, scrub: TUNE.scrub }
      });
    }
    var hero = document.querySelector('.hero');
    var glow = document.querySelector('.hero__glow');
    if (glow) {
      gsap.to(glow, {
        yPercent: TUNE.glowDrift, opacity: 0.55, ease: 'none',
        scrollTrigger: { trigger: hero, start: 'top top', end: 'bottom top', scrub: true }
      });
    }
    var copy = document.querySelector('.hero__copy');
    if (copy && conditions.full) {
      gsap.to(copy, {
        y: -50, opacity: 0.25, ease: 'none',
        scrollTrigger: { trigger: hero, start: 'top top', end: '+=560', scrub: TUNE.scrub }
      });
    }
  }

  /* ── the other windows lean back as they come in ─────────────── */

  function windows(conditions) {
    [['#team .shot .window', 0]].forEach(function (w) {
      var el = document.querySelector(w[0]);
      if (!el) return;
      var from = conditions.full
        ? { transformPerspective: 1600, y: 80, rotateX: TUNE.tilt, rotateY: w[1], scale: 0.93 }
        : { y: 36, scale: 0.97 };
      gsap.fromTo(el, from, {
        y: 0, rotateX: 0, rotateY: 0, scale: 1, ease: 'none',
        scrollTrigger: { trigger: el, start: 'top bottom', end: 'top 30%', scrub: TUNE.scrub }
      });
    });
  }

  /* ── Hangar Mobile: the phones come in one after the other ────── */

  function phones(conditions) {
    var items = Q('.phones__item');
    if (!items.length) return [];
    if (!conditions.full) return items; // a fade, with the rest
    gsap.fromTo(items, {
      transformPerspective: 1200,
      y: 120,
      z: -220,
      rotateY: function (i) { return ((items.length - 1) / 2 - i) * 9; },
      opacity: 0
    }, {
      y: 0, z: 0, rotateY: 0, opacity: 1, ease: 'none', stagger: 0.14,
      scrollTrigger: { trigger: '.phones', start: 'top 96%', end: 'top 38%', scrub: TUNE.scrub }
    });
    return [];
  }

  /* ── the landing, measured: 65 s counts up, the bars grow ───────
     Written at its end (65 s, each bar at its share of 500 s); as the
     card comes in, the bars grow from nothing and the figure counts up
     with the one of 3.0. */

  function race(conditions, cleanups) {
    var card = document.getElementById('race');
    if (!card || conditions.reduce || ScrollTrigger.isInViewport(card, 0.2)) return;
    var bars = Q('.race__bar i', card);
    var now = card.querySelector('.race__now');
    var text = now && now.firstChild;
    var counts = text && text.nodeType === 3;
    var final = counts ? text.nodeValue : '';
    var show = { v: 0 };
    // The number in a box of its own, held at its final width while it
    // counts, so the unit after it never moves.
    var num = null;
    if (counts) {
      num = document.createElement('span');
      num.className = 'race__num';
      now.insertBefore(num, text);
      num.appendChild(text);
      num.style.minWidth = num.getBoundingClientRect().width + 'px';
      text.nodeValue = '0';
    }
    gsap.set(bars, { scaleX: 0 });
    ScrollTrigger.create({
      trigger: card,
      start: 'top 78%',
      once: true,
      onEnter: function () {
        var tl = gsap.timeline({ defaults: { ease: 'power3.out' } });
        // One after the other is long; together is over quickly.
        tl.to(bars[1], { scaleX: 1, duration: 1.6, ease: 'power1.inOut' }, 0.1);
        tl.to(bars[0], { scaleX: 0.129, duration: 0.6 }, 0.1);
        if (counts) {
          tl.to(show, {
            v: parseFloat(final), duration: 0.9,
            onUpdate: function () { text.nodeValue = String(Math.round(show.v)); },
            onComplete: function () { text.nodeValue = final; }
          }, 0.1);
        }
      }
    });
    cleanups.push(function () {
      if (counts) {
        text.nodeValue = final;
        now.insertBefore(text, num);
        num.remove();
      }
      gsap.set(bars, { clearProps: 'transform' });
    });
  }

  /* ── affiliates/: the example's bars grow as it comes in ──────── */

  function affiliateBars(cleanups) {
    Q('.aff-chart').forEach(function (chart) {
      var bars = Q('.aff-bar__track i', chart);
      if (!bars.length || ScrollTrigger.isInViewport(chart, 0.2)) return;
      gsap.set(bars, { scaleX: 0 });
      ScrollTrigger.create({
        trigger: chart,
        start: 'top 80%',
        once: true,
        onEnter: function () {
          gsap.to(bars, { scaleX: 1, duration: 1.1, ease: 'power3.out', stagger: 0.09, delay: 0.15 });
        }
      });
      cleanups.push(function () { gsap.set(bars, { clearProps: 'transform' }); });
    });
  }

  /* ── links within the page glide to their section ──────────────
     The page's own smooth scrolling is off while this runs (it would
     fight ScrollTrigger); a link to a question still jumps, since
     main.js opens it. Any wheel, touch or key stops the glide. */

  var glide = null;
  function stopGlide() {
    if (glide) { glide.kill(); glide = null; }
  }
  ['wheel', 'touchstart', 'keydown'].forEach(function (type) {
    window.addEventListener(type, stopGlide, { passive: true });
  });

  document.addEventListener('click', function (e) {
    if (!allowed || e.defaultPrevented || e.button || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    var a = e.target.closest && e.target.closest('a[href^="#"]');
    var id = a && a.getAttribute('href').slice(1);
    if (!id || id === 'main') return;
    var target = document.getElementById(id);
    if (!target || target.tagName === 'DETAILS' || target.closest('details')) return;
    e.preventDefault();
    var pad = parseFloat(getComputedStyle(root).scrollPaddingTop) || 0;
    var to = id === 'top' ? 0 : target.getBoundingClientRect().top + window.scrollY - pad;
    to = gsap.utils.clamp(0, ScrollTrigger.maxScroll(window), to);
    var at = { y: window.scrollY };
    stopGlide();
    glide = gsap.to(at, {
      y: to,
      duration: gsap.utils.clamp(0.5, 1.3, Math.abs(to - at.y) / 2800),
      ease: 'power3.inOut',
      onUpdate: function () { window.scrollTo(0, at.y); },
      onComplete: function () {
        glide = null;
        if (!target.hasAttribute('tabindex') && !/^(A|BUTTON|INPUT|SELECT|TEXTAREA)$/.test(target.tagName)) {
          target.setAttribute('tabindex', '-1');
        }
        target.focus({ preventScroll: true });
      }
    });
    if (window.location.hash !== '#' + id) history.pushState(null, '', '#' + id);
  });

  /* ── one set per kind of screen ─────────────────────────────── */

  var turn = heroTurns();
  if (allowed) heroIntro(turn);
  else turn();
  root.classList.add('motion-ready');

  var mm = gsap.matchMedia();
  mm.add({
    full: '(min-width: 960px) and (prefers-reduced-motion: no-preference)',
    light: '(max-width: 959px) and (prefers-reduced-motion: no-preference)',
    reduce: '(prefers-reduced-motion: reduce)'
  }, function (context) {
    var c = context.conditions;
    var cleanups = [];
    var extra = [];
    if (!c.reduce) {
      heroScroll(c);
      windows(c);
      race(c, cleanups);
      extra = extra.concat(phones(c));
    }
    headings(c, cleanups);
    reveals(c, extra);
    if (!c.reduce) affiliateBars(cleanups);
    return function () {
      cleanups.forEach(function (fn) { fn(); });
    };
  });

  // Inter swaps in after the first layout; measure again once it has.
  if (document.fonts && document.fonts.ready) {
    document.fonts.ready.then(function () { ScrollTrigger.refresh(); });
  }
  window.addEventListener('load', function () { ScrollTrigger.refresh(); });
})();
