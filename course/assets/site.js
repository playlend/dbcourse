/* ==========================================================================
   FerricDB Course — shared page runtime (vanilla JS, no dependencies).
   Provides: theme toggle, self-check quizzes, tab panels, a step-through
   controller for animated diagrams, and tiny DOM/SVG helpers under
   window.Course. Loaded with `defer` on every page.
   ========================================================================== */
(function () {
  "use strict";

  /* ---------- theme ----------
     A tiny inline script in each page's <head> sets data-theme from
     localStorage before first paint (see templates); here we just wire
     the toggle button. */
  function initTheme() {
    var btn = document.getElementById("theme-toggle");
    if (!btn) return;
    function current() {
      var explicit = document.documentElement.getAttribute("data-theme");
      if (explicit) return explicit;
      return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
    }
    function paint() {
      btn.textContent = current() === "dark" ? "☀ Light" : "☾ Dark";
    }
    btn.addEventListener("click", function () {
      var next = current() === "dark" ? "light" : "dark";
      document.documentElement.setAttribute("data-theme", next);
      try { localStorage.setItem("ferricdb-theme", next); } catch (e) { /* file:// may block */ }
      paint();
    });
    paint();
  }

  /* ---------- quizzes ----------
     Markup convention:
       <fieldset class="quiz" data-explain="...why...">
         <legend class="quiz-q">Question text?</legend>
         <label><input type="radio" name="q-unique"> Wrong option</label>
         <label><input type="radio" name="q-unique" data-correct> Right option</label>
         <button type="button" class="quiz-check">Check answer</button>
         <div class="quiz-feedback" role="status"></div>
       </fieldset>
     Multiple data-correct options are allowed with checkboxes. */
  function initQuizzes() {
    Array.prototype.forEach.call(document.querySelectorAll(".quiz"), function (quiz) {
      var btn = quiz.querySelector(".quiz-check");
      var fb = quiz.querySelector(".quiz-feedback");
      if (!btn || !fb) return;
      btn.addEventListener("click", function () {
        var inputs = quiz.querySelectorAll("input[type=radio], input[type=checkbox]");
        var anyChecked = false, allRight = true;
        Array.prototype.forEach.call(inputs, function (inp) {
          var correct = inp.hasAttribute("data-correct");
          if (inp.checked) anyChecked = true;
          if (inp.checked !== correct) allRight = false;
          var label = inp.closest("label");
          if (label) label.classList.remove("reveal-ok", "reveal-no");
        });
        if (!anyChecked) {
          fb.className = "quiz-feedback no";
          fb.textContent = "Pick an answer first.";
          return;
        }
        var explain = quiz.getAttribute("data-explain") || "";
        fb.className = "quiz-feedback " + (allRight ? "ok" : "no");
        fb.innerHTML = (allRight ? "<strong>Correct.</strong>" : "<strong>Not quite.</strong>") +
          (explain ? '<div class="quiz-explain"></div>' : "");
        if (explain) fb.querySelector(".quiz-explain").textContent = explain;
        // On a wrong answer, reveal which selections were off; on a right
        // answer, outline the correct one(s) for reinforcement.
        Array.prototype.forEach.call(inputs, function (inp) {
          var label = inp.closest("label");
          if (!label) return;
          if (inp.hasAttribute("data-correct")) label.classList.add("reveal-ok");
          else if (inp.checked) label.classList.add("reveal-no");
        });
      });
    });
  }

  /* ---------- tabs ----------
     Markup convention:
       <div class="tabs">
         <div class="tab-list" role="tablist">
           <button role="tab" aria-selected="true">PostgreSQL</button>
           <button role="tab" aria-selected="false">SQLite</button>
         </div>
         <div class="tab-panel">...</div>
         <div class="tab-panel" hidden>...</div>
       </div> */
  function initTabs() {
    Array.prototype.forEach.call(document.querySelectorAll(".tabs"), function (tabs) {
      var btns = tabs.querySelectorAll(".tab-list [role=tab]");
      var panels = tabs.querySelectorAll(".tab-panel");
      Array.prototype.forEach.call(btns, function (btn, i) {
        btn.addEventListener("click", function () {
          Array.prototype.forEach.call(btns, function (b, j) {
            b.setAttribute("aria-selected", i === j ? "true" : "false");
            if (panels[j]) panels[j].hidden = i !== j;
          });
        });
      });
    });
  }

  /* ---------- stepper ----------
     Course.stepper(container, { total, render, captions }) builds a
     prev/next/reset control bar + caption area inside `container` and calls
     render(stepIndex) on every change. Returns { go, current }.
     Page scripts implement render() to update their own SVG/DOM. */
  function stepper(container, opts) {
    if (typeof container === "string") container = document.querySelector(container);
    if (!container) return null;
    var total = opts.total;
    var step = 0;
    var bar = document.createElement("div");
    bar.className = "stepper-bar";
    var prev = document.createElement("button");
    prev.className = "btn"; prev.type = "button"; prev.textContent = "‹ Prev";
    var next = document.createElement("button");
    next.className = "btn primary"; next.type = "button"; next.textContent = "Next ›";
    var reset = document.createElement("button");
    reset.className = "btn"; reset.type = "button"; reset.textContent = "↺ Reset";
    var label = document.createElement("span");
    label.className = "step-label";
    var caption = document.createElement("div");
    caption.className = "stepper-caption";
    caption.setAttribute("role", "status");
    bar.appendChild(prev); bar.appendChild(next); bar.appendChild(reset); bar.appendChild(label);
    container.appendChild(bar);
    container.appendChild(caption);
    function go(n) {
      step = Math.max(0, Math.min(total - 1, n));
      prev.disabled = step === 0;
      next.disabled = step === total - 1;
      label.textContent = "Step " + (step + 1) + " / " + total;
      if (opts.captions && opts.captions[step] != null) caption.textContent = opts.captions[step];
      opts.render(step, caption);
    }
    prev.addEventListener("click", function () { go(step - 1); });
    next.addEventListener("click", function () { go(step + 1); });
    reset.addEventListener("click", function () { go(0); });
    go(0);
    return { go: go, current: function () { return step; } };
  }

  /* ---------- tiny helpers ---------- */
  var SVG_NS = "http://www.w3.org/2000/svg";
  function svgEl(tag, attrs, text) {
    var el = document.createElementNS(SVG_NS, tag);
    if (attrs) for (var k in attrs) el.setAttribute(k, attrs[k]);
    if (text != null) el.textContent = text;
    return el;
  }
  function el(tag, attrs, text) {
    var node = document.createElement(tag);
    if (attrs) for (var k in attrs) {
      if (k === "class") node.className = attrs[k];
      else node.setAttribute(k, attrs[k]);
    }
    if (text != null) node.textContent = text;
    return node;
  }

  window.Course = {
    stepper: stepper,
    svgEl: svgEl,
    el: el,
    SVG_NS: SVG_NS
  };

  var booted = false;
  function boot() {
    if (booted) return;
    booted = true;
    initTheme();
    initQuizzes();
    initTabs();
  }
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot);
  } else {
    boot();
  }
})();
