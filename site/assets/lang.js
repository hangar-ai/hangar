/* ═══════════════════════════════════════════════════════════════
   HangarAI site: English or French, on the home, pricing/, buy/,
   account/, affiliates/, r/ and s/

   The inline script in each page's <head> picks the language before
   the first paint: ?lang=, then the visitor's earlier choice, then
   the browser's first language. It sets lang and data-lang on <html>,
   and pages.css (on the home, a <style> in its <head>) shows only
   that language's texts (data-l="en" or "fr"). Images keep their
   English alt and carry an aria-label with data-fr, swapped here.
   This file wires the EN / FR switch, keeps the choice, and
   tells the account area (HangarLang.on) so it draws itself again.
   ═══════════════════════════════════════════════════════════════ */
(function () {
  'use strict';

  var root = document.documentElement;
  var KEY = 'hangar.lang';
  var listeners = [];

  function get() {
    return root.dataset.lang === 'fr' ? 'fr' : 'en';
  }

  // The title, the description and the labels of the page, which an
  // attribute cannot hold twice: their French is in data-fr, and their
  // English is kept in data-en the first time.
  function swap(el, read, write, lang) {
    if (el.dataset.en === undefined) el.dataset.en = read(el);
    write(el, lang === 'fr' && el.dataset.fr !== undefined ? el.dataset.fr : el.dataset.en);
  }

  function apply(lang) {
    root.lang = lang;
    root.dataset.lang = lang;
    var title = document.querySelector('title[data-fr]');
    if (title) swap(title, function (el) { return el.textContent; }, function (el, v) { document.title = v; }, lang);
    Array.prototype.forEach.call(document.querySelectorAll('meta[data-fr]'), function (meta) {
      swap(meta, function (el) { return el.getAttribute('content'); }, function (el, v) { el.setAttribute('content', v); }, lang);
    });
    Array.prototype.forEach.call(document.querySelectorAll('[aria-label][data-fr]:not(title):not(meta)'), function (el) {
      swap(el, function (e) { return e.getAttribute('aria-label'); }, function (e, v) { e.setAttribute('aria-label', v); }, lang);
    });
    Array.prototype.forEach.call(document.querySelectorAll('[data-set-lang]'), function (button) {
      button.setAttribute('aria-pressed', String(button.dataset.setLang === lang));
    });
  }

  function set(lang) {
    if (lang !== 'en' && lang !== 'fr') return;
    try { localStorage.setItem(KEY, lang); } catch (e) { /* private window: the choice lasts this page */ }
    if (lang === get()) return;
    apply(lang);
    listeners.forEach(function (fn) { fn(lang); });
  }

  window.HangarLang = {
    get: get,
    set: set,
    on: function (fn) { listeners.push(fn); }
  };

  function wire() {
    apply(get());
    Array.prototype.forEach.call(document.querySelectorAll('[data-set-lang]'), function (button) {
      button.addEventListener('click', function () { set(button.dataset.setLang); });
    });
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', wire);
  else wire();
})();
