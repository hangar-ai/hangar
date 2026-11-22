/* ═══════════════════════════════════════════════════════════════
   HangarAI site: the header, on every page

   The header is written in each page (the same markup everywhere);
   this wires it. Without it the page still works: the bar stays
   frosted, Product opens on hover or focus, the menu button leads to
   the footer's map, and the announcement stays.

     1. the bar: clear at the top of the page, frosted with a hairline
        once the page moves under it (styles.css does it by the scroll
        where the browser can; here for the others)
     2. Product: its panel opens on hover, on a tap or from the keyboard
     3. the language: a small menu (assets/lang.js sets the language)
     4. the sheet: the nav on a phone
     5. the announcement: dismissed, remembered (the inline script in
        <head> hides it before the first paint on the next pages)
   ═══════════════════════════════════════════════════════════════ */
(function () {
  'use strict';

  var root = document.documentElement;
  var bar = document.getElementById('bar');
  if (!bar) return;
  var reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  var wide = window.matchMedia('(min-width: 960px)');

  function on(mq, fn) {
    if (mq.addEventListener) mq.addEventListener('change', fn);
    else if (mq.addListener) mq.addListener(fn);
  }

  // One menu open at a time; Escape and a click elsewhere close it.
  var openMenu = null;
  function closeOpen(focusBack) {
    if (openMenu) openMenu(false, focusBack);
  }
  document.addEventListener('keydown', function (e) {
    if (e.key === 'Escape') closeOpen(true);
  });
  document.addEventListener('pointerdown', function (e) {
    if (openMenu && !openMenu.holds(e.target)) closeOpen(false);
  });
  // The page moving on closes it too, once it has gone a little way.
  var openY = 0;
  window.addEventListener('scroll', function () {
    if (!openMenu) openY = window.scrollY;
    else if (Math.abs(window.scrollY - openY) > 48) closeOpen(false);
  }, { passive: true });

  /* ── 1. the bar ──────────────────────────────────────────────── */

  var scrollDriven = window.CSS && CSS.supports && CSS.supports('animation-timeline: scroll()');
  if (!scrollDriven) {
    var ticking = false;
    var top = function () {
      ticking = false;
      bar.classList.toggle('is-top', window.scrollY < 1);
    };
    // The first state without a transition, so nothing fades at load.
    bar.classList.add('no-anim');
    top();
    void bar.offsetHeight;
    bar.classList.remove('no-anim');
    window.addEventListener('scroll', function () {
      if (!ticking) {
        ticking = true;
        requestAnimationFrame(top);
      }
    }, { passive: true });
  }

  /* ── 2. Product ──────────────────────────────────────────────── */

  var item = bar.querySelector('.nav__item--menu');
  var trigger = item && item.querySelector('.nav__trigger');
  if (trigger) {
    var openT = 0;
    var closeT = 0;
    var openedAt = 0;
    var product = function (open, focusBack) {
      if (item.classList.contains('is-open') === open) return;
      item.classList.toggle('is-open', open);
      trigger.setAttribute('aria-expanded', String(open));
      if (open) {
        openedAt = Date.now();
        if (openMenu && openMenu !== product) openMenu(false);
        openMenu = product;
      } else {
        if (openMenu === product) openMenu = null;
        if (focusBack) trigger.focus();
      }
    };
    product.holds = function (el) { return item.contains(el); };

    // A mouse opens it by hovering, with a moment's grace on the way out.
    item.addEventListener('pointerenter', function (e) {
      if (e.pointerType !== 'mouse') return;
      clearTimeout(closeT);
      openT = setTimeout(function () { product(true); }, 70);
    });
    item.addEventListener('pointerleave', function (e) {
      if (e.pointerType !== 'mouse') return;
      clearTimeout(openT);
      closeT = setTimeout(function () { product(false); }, 200);
    });
    // A tap, a click or Enter toggles it, rather than following the link
    // (the panel's first item goes to the same place). A click right after
    // the hover opened it leaves it open.
    trigger.addEventListener('click', function (e) {
      e.preventDefault();
      var open = item.classList.contains('is-open');
      product(!open || Date.now() - openedAt < 400);
    });
    trigger.addEventListener('keydown', function (e) {
      if (e.key !== 'ArrowDown') return;
      e.preventDefault();
      product(true);
      var first = item.querySelector('.menu a');
      if (first) first.focus();
    });
    item.addEventListener('focusout', function (e) {
      if (!e.relatedTarget || !item.contains(e.relatedTarget)) product(false);
    });
    // Following one of its links closes it.
    item.querySelector('.menu').addEventListener('click', function (e) {
      if (e.target.closest('a')) product(false);
    });
  }

  /* ── 3. the language ─────────────────────────────────────────── */

  var lang = bar.querySelector('.lang');
  var langBtn = lang && lang.querySelector('.lang__btn');
  if (langBtn) {
    var language = function (open, focusBack) {
      if (lang.classList.contains('is-open') === open) return;
      lang.classList.toggle('is-open', open);
      langBtn.setAttribute('aria-expanded', String(open));
      if (open) {
        if (openMenu && openMenu !== language) openMenu(false);
        openMenu = language;
        var pressed = lang.querySelector('[aria-pressed="true"]');
        if (pressed) pressed.focus();
      } else {
        if (openMenu === language) openMenu = null;
        if (focusBack) langBtn.focus();
      }
    };
    language.holds = function (el) { return lang.contains(el); };
    langBtn.addEventListener('click', function () {
      language(!lang.classList.contains('is-open'));
    });
    lang.addEventListener('click', function (e) {
      if (e.target.closest('[data-set-lang]')) language(false, true);
    });
    lang.addEventListener('focusout', function (e) {
      if (!e.relatedTarget || !lang.contains(e.relatedTarget)) language(false);
    });
  }

  /* ── 4. the sheet ────────────────────────────────────────────── */

  var sheet = document.getElementById('sheet');
  var menuBtn = bar.querySelector('.bar__menu');
  if (sheet && menuBtn) {
    var hideT = 0;
    var setSheet = function (open, instant) {
      if ((menuBtn.getAttribute('aria-expanded') === 'true') === open) return;
      clearTimeout(hideT);
      menuBtn.setAttribute('aria-expanded', String(open));
      if (open) {
        closeOpen(false);
        // Under the bar wherever it is (the announcement may be above it).
        sheet.style.setProperty('--sheet-top', Math.max(0, Math.round(bar.getBoundingClientRect().bottom)) + 'px');
        sheet.hidden = false;
        root.classList.add('sheet-open');
        void sheet.offsetHeight;
        sheet.classList.add('is-open');
      } else {
        sheet.classList.remove('is-open');
        root.classList.remove('sheet-open');
        if (instant || reduced) sheet.hidden = true;
        else hideT = setTimeout(function () { sheet.hidden = true; }, 220);
      }
    };
    menuBtn.addEventListener('click', function (e) {
      e.preventDefault();
      setSheet(menuBtn.getAttribute('aria-expanded') !== 'true');
    });
    // It is a link to the footer's map without script: Space works as on a button.
    menuBtn.addEventListener('keydown', function (e) {
      if (e.key !== ' ') return;
      e.preventDefault();
      menuBtn.click();
    });
    sheet.addEventListener('click', function (e) {
      if (e.target.closest('a')) setSheet(false, true);
    });
    document.addEventListener('keydown', function (e) {
      if (e.key === 'Escape' && menuBtn.getAttribute('aria-expanded') === 'true') {
        setSheet(false);
        menuBtn.focus();
      }
    });
    on(wide, function () {
      if (wide.matches) setSheet(false, true);
    });
  }
  on(wide, function () { closeOpen(false); });

  /* ── 5. the announcement ─────────────────────────────────────── */

  var announce = document.getElementById('announce');
  var dismiss = announce && announce.querySelector('.announce__x');
  if (dismiss) {
    dismiss.addEventListener('click', function () {
      var id = announce.getAttribute('data-id') || '';
      try { localStorage.setItem('hangar.announce', id); } catch (e) { /* storage refused: gone for this page */ }
      var done = function () {
        clearTimeout(late);
        announce.removeEventListener('transitionend', end);
        root.setAttribute('data-announce-seen', id);
        announce.classList.remove('is-closing');
        announce.removeAttribute('style');
        // What the scroll drives sits higher now.
        if (window.ScrollTrigger) window.ScrollTrigger.refresh();
      };
      var end = function (e) {
        if (e.target === announce && e.propertyName === 'height') done();
      };
      var late = 0;
      if (reduced) {
        done();
        return;
      }
      announce.style.height = announce.offsetHeight + 'px';
      announce.classList.add('is-closing');
      void announce.offsetHeight;
      announce.style.height = '0px';
      announce.style.opacity = '0';
      announce.addEventListener('transitionend', end);
      late = setTimeout(done, 500);
    });
  }
})();
