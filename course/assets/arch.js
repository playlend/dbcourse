/* ==========================================================================
   FerricDB Course — shared architecture diagram.
   Course.renderArchitecture(container, opts) draws the full-system SVG.
   Every milestone page calls this with `done` / `current` so the same
   diagram progressively lights up as the project is built.

   opts:
     done:    array of component ids already built (green)
     current: array of component ids being built now (blue highlight)
     root:    relative path from the page to the course root (default "../..")
     links:   if true (default), each box links to its concept module page
     title:   accessible <title> text for the SVG
   Component ids: client, protocol, frontend, catalog, planner, executor,
                  txn, heap, index, buffer, wal, disk
   Requires site.js (Course.svgEl) to be loaded first.
   ========================================================================== */
(function () {
  "use strict";

  var COMPONENTS = [
    { id: "client",   x: 240, y: 16,  w: 240, h: 48, label: "Client (psql, drivers)",  sub: "speaks the PG wire protocol", module: "m11-wire-protocol-server" },
    { id: "protocol", x: 240, y: 92,  w: 240, h: 48, label: "Wire Protocol Server",    sub: "M11 · Milestone 10",          module: "m11-wire-protocol-server" },
    { id: "frontend", x: 240, y: 168, w: 240, h: 48, label: "SQL Front-End",           sub: "lexer · parser · binder — M05 · MS3", module: "m05-sql-parsing" },
    { id: "planner",  x: 240, y: 244, w: 240, h: 48, label: "Planner / Optimizer",     sub: "M06 + M08 · MS4 + MS7",       module: "m08-query-optimization" },
    { id: "executor", x: 240, y: 320, w: 240, h: 48, label: "Executor (Volcano)",      sub: "M06–M07 · MS4 + MS6",         module: "m06-query-execution" },
    { id: "heap",     x: 170, y: 396, w: 180, h: 48, label: "Heap Storage",            sub: "M02 · MS1",                   module: "m02-storage" },
    { id: "index",    x: 370, y: 396, w: 180, h: 48, label: "B+Tree Index",            sub: "M04 · MS5",                   module: "m04-data-structures-indexes" },
    { id: "buffer",   x: 240, y: 472, w: 240, h: 48, label: "Buffer Pool",             sub: "M03 · MS2",                   module: "m03-buffer-pool" },
    { id: "disk",     x: 240, y: 548, w: 240, h: 48, label: "Disk Manager & Files",    sub: "M02 · MS1",                   module: "m02-storage" },
    { id: "catalog",  x: 640, y: 168, w: 220, h: 48, label: "Catalog",                 sub: "M05 · MS3",                   module: "m05-sql-parsing" },
    { id: "txn",      x: 640, y: 320, w: 220, h: 48, label: "Transaction Manager",     sub: "MVCC — M09 · MS8",            module: "m09-transactions-mvcc" },
    { id: "wal",      x: 640, y: 472, w: 220, h: 48, label: "WAL / Recovery",          sub: "M10 · MS9",                   module: "m10-logging-recovery" }
  ];

  // Edges as SVG path data (elbow connectors between box edges), plus an
  // optional small label. `arrow: "both"` puts heads on both ends.
  var EDGES = [
    { d: "M 360 64 V 92" },
    { d: "M 360 140 V 168" },
    { d: "M 360 216 V 244" },
    { d: "M 360 292 V 320" },
    { d: "M 330 368 V 382 H 260 V 396" },
    { d: "M 390 368 V 382 H 460 V 396" },
    { d: "M 260 444 V 458 H 330 V 472" },
    { d: "M 460 444 V 458 H 390 V 472" },
    { d: "M 360 520 V 548" },
    { d: "M 480 192 H 640", arrow: "both", label: { x: 560, y: 184, text: "name / type lookup" } },
    { d: "M 480 268 H 750 V 216", label: { x: 620, y: 260, text: "statistics" } },
    { d: "M 480 344 H 640", arrow: "both", label: { x: 560, y: 336, text: "visibility, conflicts" } },
    { d: "M 750 368 V 472", label: { x: 760, y: 424, anchor: "start", text: "commit / abort records" } },
    { d: "M 480 496 H 640", dashed: true, label: { x: 560, y: 488, text: "WAL rule: log first" } },
    { d: "M 750 520 V 572 H 480", label: { x: 620, y: 564, text: "log file" } }
  ];

  var markerSeq = 0;

  function renderArchitecture(container, opts) {
    opts = opts || {};
    if (typeof container === "string") container = document.querySelector(container);
    if (!container || !window.Course) return;
    var S = Course.svgEl;
    var root = opts.root == null ? "../.." : opts.root;
    var done = opts.done || [];
    var current = opts.current || [];
    var useLinks = opts.links !== false;
    var hasProgress = done.length > 0 || current.length > 0;

    var svg = S("svg", {
      viewBox: "0 0 920 " + (hasProgress ? 640 : 604),
      role: "img",
      "font-family": "inherit"
    });
    var t = S("title", null, opts.title ||
      "FerricDB architecture: client, wire protocol, SQL front-end, planner, executor, access methods, buffer pool, disk, with catalog, transaction manager, and WAL at the side.");
    svg.appendChild(t);

    // arrowhead markers (unique ids so several diagrams can share a page)
    var mid = "arch-arrow-" + (markerSeq++);
    var defs = S("defs");
    ["", "-rev"].forEach(function (suffix) {
      var m = S("marker", {
        id: mid + suffix, viewBox: "0 0 10 10", refX: 9, refY: 5,
        markerWidth: 7, markerHeight: 7,
        orient: suffix ? "auto-start-reverse" : "auto"
      });
      var p = S("path", { d: "M 0 0 L 10 5 L 0 10 z" });
      p.setAttribute("fill", "currentColor");
      p.setAttribute("opacity", "0.65");
      m.appendChild(p);
      defs.appendChild(m);
    });
    svg.appendChild(defs);

    EDGES.forEach(function (e) {
      var path = S("path", { d: e.d, "class": "d-line" });
      path.setAttribute("marker-end", "url(#" + mid + ")");
      if (e.arrow === "both") path.setAttribute("marker-start", "url(#" + mid + "-rev)");
      if (e.dashed) path.setAttribute("stroke-dasharray", "5 4");
      svg.appendChild(path);
      if (e.label) {
        var lbl = S("text", { x: e.label.x, y: e.label.y, "class": "d-text-sm", "text-anchor": e.label.anchor || "middle" }, e.label.text);
        svg.appendChild(lbl);
      }
    });

    COMPONENTS.forEach(function (c) {
      var cls = "d-box";
      if (current.indexOf(c.id) !== -1) cls = "d-box-hl";
      else if (done.indexOf(c.id) !== -1) cls = "d-box-good";
      var g = S("g");
      g.appendChild(S("rect", { x: c.x, y: c.y, width: c.w, height: c.h, rx: 8, "class": cls }));
      g.appendChild(S("text", {
        x: c.x + c.w / 2, y: c.y + 20, "class": "d-text",
        "text-anchor": "middle", "font-weight": "600"
      }, c.label));
      g.appendChild(S("text", {
        x: c.x + c.w / 2, y: c.y + 37, "class": "d-text-sm", "text-anchor": "middle"
      }, c.sub));
      if (useLinks && c.module) {
        var a = S("a", { href: root + "/modules/" + c.module + "/index.html" });
        a.appendChild(g);
        svg.appendChild(a);
      } else {
        svg.appendChild(g);
      }
    });

    if (hasProgress) {
      var legend = S("g");
      var items = [
        { cls: "d-box-good", text: "built in earlier milestones", x: 240 },
        { cls: "d-box-hl", text: "this milestone", x: 480 },
        { cls: "d-box", text: "not built yet", x: 650 }
      ];
      items.forEach(function (it) {
        legend.appendChild(S("rect", { x: it.x, y: 612, width: 18, height: 14, rx: 3, "class": it.cls }));
        legend.appendChild(S("text", { x: it.x + 24, y: 624, "class": "d-text-sm" }, it.text));
      });
      svg.appendChild(legend);
    }

    container.appendChild(svg);
    return svg;
  }

  window.Course = window.Course || {};
  window.Course.renderArchitecture = renderArchitecture;
})();
