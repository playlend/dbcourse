#!/usr/bin/env python3
"""Deterministic QA checker for the FerricDB course pages.

Checks every .html file under course/ against the mechanical parts of
authoring/CONVENTIONS.md:

  1. offline rule: no external <script src>/<link href>/<img src>/url(...)
  2. every external citation URL is on the allowlist
  3. internal relative links resolve to existing files
  4. same-page fragment links point at existing ids
  5. every sup.cite anchor resolves to an existing reference id
  6. exactly one <h1>; <html lang>; <title>; viewport meta
  7. theme pre-paint script present; stylesheet + site.js wired with the
     right relative depth
  8. quiz sanity: each .quiz has a quiz-check button, a quiz-feedback div,
     inputs with a shared name, and at least one data-correct
  9. footer structure on content pages: Primary sources before Go deeper
     before Milestone link (modules) before prevnext

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
        self._stack = []

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if "id" in a:
            self.ids.add(a["id"])
        cls = a.get("class", "")
        if tag == "h1":
            self.h1_count += 1
        elif tag == "html":
            self.lang = a.get("lang")
        elif tag == "title":
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
        elif tag == "sup" and "cite" in cls.split():
            self._in_sup_cite = True
            self._stack.append("sup")
        if tag == "fieldset" and "quiz" in cls.split():
            self._quiz = {"names": set(), "correct": 0, "check": False,
                          "feedback": False, "inputs": 0}
        if self._quiz is not None:
            if tag == "input":
                self._quiz["inputs"] += 1
                if "name" in a:
                    self._quiz["names"].add(a["name"])
                if "data-correct" in a:
                    self._quiz["correct"] += 1
            if tag == "button" and "quiz-check" in cls.split():
                self._quiz["check"] = True
            if tag == "div" and "quiz-feedback" in cls.split():
                self._quiz["feedback"] = True

    def handle_endtag(self, tag):
        if tag == "sup" and self._stack and self._stack[-1] == "sup":
            self._stack.pop()
            self._in_sup_cite = False
        if tag == "fieldset" and self._quiz is not None:
            self.quizzes.append(self._quiz)
            self._quiz = None


def check_page(path: Path):
    findings = []
    text = path.read_text(encoding="utf-8", errors="replace")
    p = PageParser()
    p.feed(text)

    rel = path.relative_to(COURSE)
    depth = len(rel.parts) - 1  # index.html -> 0, project/index.html -> 1
    prefix = "../" * depth

    # 1. offline
    for load in p.external_loads:
        findings.append(f"external resource load: {load}")
    for m in re.finditer(r"url\(\s*['\"]?(https?:)?//", text):
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

    # 7. theme + assets wiring
    if "ferricdb-theme" not in text:
        findings.append("missing theme pre-paint script")
    want_css = prefix + "assets/style.css"
    if want_css not in [s for s in p.stylesheets]:
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

    # 5. citations
    for href in p.cite_targets:
        if not href.startswith("#"):
            findings.append(f"sup.cite links non-fragment: {href}")
        elif href[1:] not in p.ids:
            findings.append(f"citation target missing: {href}")

    # 8. quizzes
    for i, q in enumerate(p.quizzes, 1):
        if not q["check"]:
            findings.append(f"quiz {i}: missing quiz-check button")
        if not q["feedback"]:
            findings.append(f"quiz {i}: missing quiz-feedback div")
        if q["inputs"] and q["correct"] == 0:
            findings.append(f"quiz {i}: no data-correct option")
        if len(q["names"]) > 1:
            findings.append(f"quiz {i}: mixed input names {sorted(q['names'])}")

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
        present = [(l, pos) for l, pos in order if pos is not None]
        if present != sorted(present, key=lambda t: t[1]):
            findings.append("footer sections out of order: " +
                            " ".join(l for l, _ in present))
        if order[3][1] is None:
            findings.append("missing prev/next nav")

    return findings


def collect_external_urls():
    urls = set()
    for f in COURSE.rglob("*.html"):
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
    pages = sorted(COURSE.rglob("*.html"))
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
