#!/usr/bin/env python3
"""Deterministic QA checker for the FerricDB course pages.

Checks every .html file under course/ against the mechanical parts of
authoring/CONVENTIONS.md:

  1. offline rule: no external <script src>/<link href>/<img src>/url(...)
  2. every external citation URL is on the allowlist
  3. internal relative links resolve to existing files
  4. same-page fragment links point at existing ids; no duplicate ids
  5. every sup.cite anchor resolves to an existing reference id, and every
     ref-N in Primary sources is actually cited in body prose
  6. exactly one <h1>; <html lang>; <title>; viewport meta
  7. theme pre-paint script present; stylesheet + site.js wired with the
     right relative depth
  8. quiz sanity: each .quiz has a quiz-check button, a quiz-feedback div,
     inputs with a shared name (unique across quizzes on a page), and
     exactly one data-correct for radio groups (or >=1 for checkboxes)
  9. footer structure on content pages: Primary sources before Go deeper
     before Milestone link (modules only) before prevnext; project pages
     must not carry a Milestone link section
 10. SVG & table hygiene: inline <svg> has role="img", <title>, viewBox
     width <= 960, zero hardcoded hex/rgb/hsl colors; every <table> sits
     inside .table-wrap
 11. TOC completeness: every main <h2> id (plus #sec-sources) is linked
     in .toc when a .toc box is present
 12. Voice rule: no forbidden "simply" in prose (outside code/script/style)

Usage:  python3 authoring/qa_check.py [--net]
        --net additionally HEAD-checks every distinct external URL
              (dl.acm.org is skipped: it 403s non-browser clients by design)

Exit code 0 = clean, 1 = findings.
"""

import re
import sys
import subprocess
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import urlparse, unquote

COURSE = Path(__file__).resolve().parent.parent / "course"

ALLOWED_HOSTS = {
    "15445.courses.cs.cmu.edu",
    "cs186berkeley.net", "www.cs186berkeley.net",
    "www.interdb.jp", "interdb.jp",
    "dsf.berkeley.edu",
    "redbook.io", "www.redbook.io",
    "www.postgresql.org", "postgresql.org",
    "sqlite.org", "www.sqlite.org",
    "web.stanford.edu",
    "dl.acm.org",
    "users.cs.utah.edu",
    "db.cs.cmu.edu",
    "arxiv.org",
    "cstack.github.io",
    "github.com", "raw.githubusercontent.com",
    "skyzh.github.io",
    "docs.rs",
    "doc.rust-lang.org",
}

NET_SKIP_HOSTS = {"dl.acm.org"}  # 403 to non-browser clients, by design


class PageParser(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.ids = set()
        self.dup_ids = []
        self.links = []          # (href, is_citation_context)
        self.h1_count = 0
        self.lang = None
        self.has_title = False
        self.has_viewport = False
        self.external_loads = []  # script/link/img/source pulling remote
        self.scripts_local = []
        self.stylesheets = []
        self.quizzes = []         # list of dicts
        self._quiz = None
        self._in_sup_cite = False
        self.cite_targets = []
        self.ref_ids = []
        self._stack = []
        # SVG & table checks
        self._in_svg = 0
        self._in_svg_style = False
        self._svg_role = False
        self._svg_title = False
        self.svg_issues = []
        self._table_wrap_depth = 0
        self._div_stack = []
        self.unwrapped_tables = 0
        # TOC & heading tracking
        self._in_footer = False
        self._toc_depth = 0
        self.has_toc = False
        self.toc_hrefs = set()
        self.body_h2s = []       # list of id (or None if missing id)
        # Prose style check (exclude script/style/code/pre/svg)
        self._prose_skip_depth = 0
        self.simply_hits = 0

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if "id" in a:
            if a["id"] in self.ids:
                self.dup_ids.append(a["id"])
            self.ids.add(a["id"])
            if tag == "li" and re.fullmatch(r"ref-\d+", a["id"]):
                self.ref_ids.append(a["id"])
        cls = a.get("class", "")
        cls_list = cls.split()

        if tag in ("script", "style", "code", "pre", "svg"):
            self._prose_skip_depth += 1

        if tag == "footer":
            self._in_footer = True
        elif tag == "h2" and not self._in_footer:
            self.body_h2s.append(a.get("id"))

        if tag == "div":
            is_tw = "table-wrap" in cls_list
            is_toc = "toc" in cls_list
            self._div_stack.append((is_tw, is_toc))
            if is_tw:
                self._table_wrap_depth += 1
            if is_toc:
                self.has_toc = True
                self._toc_depth += 1
        elif tag == "table":
            if self._table_wrap_depth == 0:
                self.unwrapped_tables += 1

        if tag == "svg":
            self._in_svg += 1
            self._svg_role = a.get("role") == "img"
            self._svg_title = False
            vb = a.get("viewbox", "").strip()
            parts = [pt for pt in re.split(r"[\s,]+", vb) if pt] if vb else []
            if len(parts) == 4:
                try:
                    w = float(parts[2])
                    if w > 960:
                        self.svg_issues.append(f"svg viewBox width {w:g} > 960")
                except ValueError:
                    pass
            elif "width" in a:
                try:
                    w = float(re.sub(r"px$", "", a["width"].strip()))
                    if w > 960:
                        self.svg_issues.append(f"svg width {w:g} > 960")
                except ValueError:
                    pass
        if self._in_svg > 0:
            if tag == "title":
                self._svg_title = True
            if tag == "style":
                self._in_svg_style = True
            for k in ("fill", "stroke", "color", "stop-color", "flood-color"):
                v = a.get(k, "").strip()
                if v and not re.fullmatch(
                    r"(none|currentColor|transparent|inherit|var\(--[^)]+\)|url\(#[^)]+\))",
                    v,
                    re.IGNORECASE,
                ):
                    self.svg_issues.append(f"svg hardcoded color in {tag} {k}={v!r}")
            style_v = a.get("style", "")
            if re.search(r"#[0-9a-fA-F]{3,8}|rgba?\(|hsla?\(", style_v, re.IGNORECASE):
                self.svg_issues.append(f"svg hardcoded color in {tag} style={style_v!r}")

        if tag == "h1":
            self.h1_count += 1
        elif tag == "html":
            self.lang = a.get("lang")
        elif tag == "title" and self._in_svg == 0:
            self.has_title = True
        elif tag == "meta" and a.get("name") == "viewport":
            self.has_viewport = True
        elif tag == "script":
            src = a.get("src")
            if src:
                if src.startswith(("http://", "https://", "//")):
                    self.external_loads.append(f"script src={src}")
                else:
                    self.scripts_local.append(src)
        elif tag == "link":
            href = a.get("href", "")
            if href.startswith(("http://", "https://", "//")):
                self.external_loads.append(f"link href={href}")
            elif "stylesheet" in a.get("rel", ""):
                self.stylesheets.append(href)
        elif tag in ("img", "source", "iframe", "video", "audio", "embed", "object"):
            src = a.get("src") or a.get("data") or ""
            if src.startswith(("http://", "https://", "//")):
                self.external_loads.append(f"{tag} src={src}")
        elif tag == "a":
            href = a.get("href")
            if href:
                self.links.append((href, self._in_sup_cite))
                if self._in_sup_cite:
                    self.cite_targets.append(href)
                if self._toc_depth > 0 and href.startswith("#"):
                    self.toc_hrefs.add(href[1:])
        elif tag == "sup" and "cite" in cls_list:
            self._in_sup_cite = True
            self._stack.append("sup")
        if tag == "fieldset" and "quiz" in cls_list:
            self._quiz = {"names": set(), "types": set(), "bad_types": [],
                          "missing_name": 0, "correct": 0,
                          "check": False, "feedback": False, "inputs": 0}
            explain = a.get("data-explain", "")
            if explain:
                self.simply_hits += len(re.findall(r"\bsimply\b", explain, re.IGNORECASE))
        if self._quiz is not None:
            if tag == "input":
                self._quiz["inputs"] += 1
                itype = a.get("type", "").strip().lower()
                if itype in ("radio", "checkbox"):
                    self._quiz["types"].add(itype)
                else:
                    self._quiz["bad_types"].append(itype or "<missing>")
                nm = a.get("name", "").strip()
                if nm:
                    self._quiz["names"].add(nm)
                else:
                    self._quiz["missing_name"] += 1
                if "data-correct" in a:
                    self._quiz["correct"] += 1
            if tag == "button" and "quiz-check" in cls_list:
                self._quiz["check"] = True
            if tag == "div" and "quiz-feedback" in cls_list:
                self._quiz["feedback"] = True

    def handle_endtag(self, tag):
        if tag in ("script", "style", "code", "pre", "svg") and self._prose_skip_depth > 0:
            self._prose_skip_depth -= 1
        if tag == "footer":
            self._in_footer = False
        if tag == "style" and self._in_svg_style:
            self._in_svg_style = False
        if tag == "div" and self._div_stack:
            is_tw, is_toc = self._div_stack.pop()
            if is_tw and self._table_wrap_depth > 0:
                self._table_wrap_depth -= 1
            if is_toc and self._toc_depth > 0:
                self._toc_depth -= 1
        if tag == "svg" and self._in_svg > 0:
            if not self._svg_role:
                self.svg_issues.append("inline <svg> missing role=\"img\"")
            if not self._svg_title:
                self.svg_issues.append("inline <svg> missing <title>")
            self._in_svg -= 1
        if tag == "sup" and self._stack and self._stack[-1] == "sup":
            self._stack.pop()
            self._in_sup_cite = False
        if tag == "fieldset" and self._quiz is not None:
            self.quizzes.append(self._quiz)
            self._quiz = None

    def handle_data(self, data):
        if self._in_svg_style:
            if re.search(r"#[0-9a-fA-F]{3,8}|rgba?\(|hsla?\(", data, re.IGNORECASE):
                self.svg_issues.append("svg hardcoded color inside <style>")
        if self._prose_skip_depth == 0:
            self.simply_hits += len(re.findall(r"\bsimply\b", data, re.IGNORECASE))


def check_page(path: Path):
    findings = []
    text = path.read_text(encoding="utf-8", errors="replace")
    p = PageParser()
    p.feed(text)

    rel = path.relative_to(COURSE)
    depth = len(rel.parts) - 1  # index.html -> 0, project/index.html -> 1
    prefix = "../" * depth
    is_module = rel.parts[0] == "modules" if depth == 2 else False
    is_project = rel.parts[0] == "project" if depth == 2 else False

    # 1. offline
    for load in p.external_loads:
        findings.append(f"external resource load: {load}")
    for _m in re.finditer(r"url\(\s*['\"]?(https?:)?//", text):
        findings.append("css url() pulling a remote resource")
    if re.search(r"@import\s+url\(\s*['\"]?https?:", text):
        findings.append("css @import of a remote stylesheet")

    # 6. structure
    if p.h1_count != 1:
        findings.append(f"expected exactly 1 <h1>, found {p.h1_count}")
    if p.lang != "en":
        findings.append("missing html lang=en")
    if not p.has_title:
        findings.append("missing <title>")
    if not p.has_viewport:
        findings.append("missing viewport meta")
    for dup in p.dup_ids:
        findings.append(f"duplicate id: #{dup}")

    # 7. theme + assets wiring
    if "ferricdb-theme" not in text:
        findings.append("missing theme pre-paint script")
    want_css = prefix + "assets/style.css"
    if want_css not in list(p.stylesheets):
        findings.append(f"stylesheet not wired as {want_css} "
                        f"(found: {p.stylesheets})")
    want_js = prefix + "assets/site.js"
    if want_js not in p.scripts_local:
        findings.append(f"site.js not wired as {want_js} "
                        f"(found: {p.scripts_local})")

    # 2/3/4. links
    for href, _in_cite in p.links:
        if href.startswith(("http://", "https://")):
            host = urlparse(href).netloc
            if host not in ALLOWED_HOSTS:
                findings.append(f"non-allowlisted external URL: {href}")
        elif href.startswith("#"):
            if href[1:] and href[1:] not in p.ids:
                findings.append(f"broken fragment link: {href}")
        elif href.startswith(("mailto:", "javascript:")):
            findings.append(f"suspicious link scheme: {href}")
        else:
            target = href.split("#", 1)
            f = (path.parent / unquote(target[0])).resolve()
            if not f.exists():
                findings.append(f"broken internal link: {href}")

    # 5. citations (both directions: every cite exists, every ref is cited)
    cited_ids = set()
    for href in p.cite_targets:
        if not href.startswith("#"):
            findings.append(f"sup.cite links non-fragment: {href}")
        elif href[1:] not in p.ids:
            findings.append(f"citation target missing: {href}")
        else:
            cited_ids.add(href[1:])
    for ref_id in p.ref_ids:
        if ref_id not in cited_ids:
            findings.append(f"unused reference in Primary sources: #{ref_id}")

    # 8. quizzes
    seen_quiz_names = set()
    for i, q in enumerate(p.quizzes, 1):
        if not q["check"]:
            findings.append(f"quiz {i}: missing quiz-check button")
        if not q["feedback"]:
            findings.append(f"quiz {i}: missing quiz-feedback div")
        if q["inputs"] == 0:
            findings.append(f"quiz {i}: no <input> options")
        if q["bad_types"]:
            findings.append(f"quiz {i}: invalid/missing input type(s): {q['bad_types']}")
        if len(q["types"]) > 1:
            findings.append(f"quiz {i}: mixed input types {sorted(q['types'])}")
        if q["missing_name"]:
            findings.append(f"quiz {i}: {q['missing_name']} input(s) missing name attribute")
        if q["inputs"] and q["correct"] == 0:
            findings.append(f"quiz {i}: no data-correct option")
        if "radio" in q["types"] and q["correct"] != 1:
            findings.append(f"quiz {i}: radio quiz must have exactly 1 data-correct (found {q['correct']})")
        if len(q["names"]) > 1:
            findings.append(f"quiz {i}: mixed input names {sorted(q['names'])}")
        for nm in q["names"]:
            if nm in seen_quiz_names:
                findings.append(f"quiz {i}: duplicate input name across quizzes: {nm}")
            seen_quiz_names.add(nm)

    # 9. footer ordering (content pages only)
    if depth == 2:
        order = []
        for label, pat in [("sources", r"<h2[^>]*>Primary sources"),
                           ("deeper", r"<h2[^>]*>Go deeper"),
                           ("milestone", r"<h2[^>]*>Milestone link"),
                           ("prevnext", r'class="prevnext"')]:
            m = re.search(pat, text)
            order.append((label, m.start() if m else None))
        if order[0][1] is None:
            findings.append("missing 'Primary sources' section")
        if order[1][1] is None:
            findings.append("missing 'Go deeper' section")
        if is_module and order[2][1] is None:
            findings.append("module missing 'Milestone link' section")
        if is_project and order[2][1] is not None:
            findings.append("project page must not have a 'Milestone link' section (modules only)")
        present = [(l, pos) for l, pos in order if pos is not None]
        if present != sorted(present, key=lambda t: t[1]):
            findings.append("footer sections out of order: " +
                            " ".join(l for l, _ in present))
        if order[3][1] is None:
            findings.append("missing prev/next nav")

    # 10. SVG & table hygiene
    for issue in p.svg_issues:
        findings.append(issue)
    if p.unwrapped_tables:
        findings.append(f"{p.unwrapped_tables} <table> element(s) not wrapped in .table-wrap")

    # 11. TOC completeness (every non-footer <h2> plus #sec-sources must appear in .toc)
    if depth == 2 and not p.has_toc:
        findings.append("content page missing .toc box")
    if p.has_toc:
        missing_h2_ids = sum(1 for hid in p.body_h2s if not hid)
        if missing_h2_ids:
            findings.append(f"{missing_h2_ids} body <h2> heading(s) missing id attribute")
        for hid in p.body_h2s:
            if hid and hid not in p.toc_hrefs:
                findings.append(f"TOC missing body heading #{hid}")
        if depth == 2:
            if "sec-sources" not in p.ids:
                findings.append("Primary sources heading missing id=\"sec-sources\"")
            elif "sec-sources" not in p.toc_hrefs:
                findings.append("TOC missing #sec-sources")

    # 12. Voice rule + unclosed skip tags
    if p._prose_skip_depth > 0:
        findings.append("unclosed <script>/<style>/<code>/<pre>/<svg> tag at EOF")
    if p.simply_hits:
        findings.append(f"forbidden word 'simply' appears {p.simply_hits} time(s) in prose")

    return findings


def course_pages():
    return sorted(
        f for f in COURSE.rglob("*.html")
        if "ferricdb" not in f.relative_to(COURSE).parts
    )


def collect_external_urls():
    urls = set()
    for f in course_pages():
        for m in re.finditer(r'href="(https?://[^"]+)"', f.read_text(
                encoding="utf-8", errors="replace")):
            urls.add(m.group(1))
    return sorted(urls)


def net_check(urls):
    bad = []
    for u in urls:
        host = urlparse(u).netloc
        if host in NET_SKIP_HOSTS:
            continue
        try:
            r = subprocess.run(
                ["curl", "-sIL", "-o", "/dev/null", "-w", "%{http_code}",
                 "--max-time", "20", "-A",
                 "Mozilla/5.0 (X11; Linux x86_64) course-link-check", u],
                capture_output=True, text=True, timeout=30)
            code = r.stdout.strip()
            if not code.startswith(("2", "3")):
                # some servers reject HEAD; retry with GET
                r = subprocess.run(
                    ["curl", "-sL", "-o", "/dev/null", "-w", "%{http_code}",
                     "--max-time", "25", "-A",
                     "Mozilla/5.0 (X11; Linux x86_64) course-link-check", u],
                    capture_output=True, text=True, timeout=35)
                code = r.stdout.strip()
            if not code.startswith(("2", "3")):
                bad.append((u, code))
        except Exception as e:  # noqa: BLE001
            bad.append((u, f"error {e}"))
    return bad


def main():
    total = 0
    pages = course_pages()
    if not pages:
        print("no pages found under", COURSE)
        return 1
    for f in pages:
        findings = check_page(f)
        if findings:
            total += len(findings)
            print(f"\n== {f.relative_to(COURSE.parent)} ==")
            for x in findings:
                print("  -", x)
    print(f"\nchecked {len(pages)} pages: "
          f"{'CLEAN' if total == 0 else str(total) + ' findings'}")

    if "--net" in sys.argv:
        urls = collect_external_urls()
        print(f"\nnet-checking {len(urls)} distinct external URLs ...")
        bad = net_check(urls)
        for u, code in bad:
            print(f"  DEAD? [{code}] {u}")
        print("net check:", "CLEAN" if not bad else f"{len(bad)} problems")
        total += len(bad)

    return 0 if total == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
