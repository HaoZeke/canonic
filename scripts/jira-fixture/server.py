#!/usr/bin/env python3
"""Minimal Jira Server REST API v2 fixture for canonic import-jira tests.

Implements only what canonic uses:
  GET /rest/api/2/search?jql=...&startAt=&maxResults=&fields=summary
  GET /rest/api/2/issue/{key}/comment

Seed data mirrors the Support meeting direction: messy personal canned answers
living as issue comments, to be pulled as review drafts (never auto-promoted).
"""

from __future__ import annotations

import base64
import json
import re
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

HOST = "0.0.0.0"
PORT = 8080

# Disposable fixture-only credentials (local container smoke — not production secrets).
USER = "advisor"
PASSWORD = "canonic-test"
# Also accept a Bearer PAT so both canonic auth modes can be exercised.
PAT = "pat-canonic-fixture-token"

# --- seed corpus (wiki markup bodies, not markdown) -----------------------

ISSUES = [
    {
        "key": "HSP-101",
        "summary": "Project space is not a backup",
        "labels": ["canned-response", "storage"],
        "comments": [
            {
                "author": "Alice Advisor",
                "created": "2025-11-02T10:15:00.000+0000",
                "body": (
                    "h1. Project space is not a backup\n\n"
                    "Demo *project space* is for active working data, not "
                    "long-term archival. Use the self-service backup options "
                    "described in the storage SOP.\n\n"
                    "Regards,\nAlice Advisor\n"  # personal sign-off (quality fail)
                ),
            },
            {
                "author": "Bob Firstline",
                "created": "2026-01-14T09:00:00.000+0000",
                "body": (
                    "Also remind users that {{/home}} and scratch are separate "
                    "quotas. See the *storage* FAQ.\n"
                ),
            },
        ],
    },
    {
        "key": "HSP-102",
        "summary": "How to request a software install on the cluster",
        "labels": ["canned-response", "software"],
        "comments": [
            {
                "author": "Carol Stack",
                "created": "2026-03-01T12:00:00.000+0000",
                "body": (
                    "h2. Software install requests\n\n"
                    "Please open a ticket with:\n"
                    "* package name and version\n"
                    "* why the central module stack is insufficient\n"
                    "* license constraints if any\n\n"
                    "We prefer EasyBuild easyconfigs when available.\n\n"
                    "Regards,\nSupport Team\n"
                ),
            }
        ],
    },
    {
        "key": "HSP-103",
        "summary": "Unrelated networking question",
        "labels": ["networking"],  # no canned-response — must not match JQL
        "comments": [
            {
                "author": "Dave Net",
                "created": "2026-04-01T08:00:00.000+0000",
                "body": "Please use eduVPN for off-site access.\n",
            }
        ],
    },
    {
        "key": "HSP-104",
        "summary": "Project space backup policy (stale duplicate)",
        "labels": ["canned-response", "storage"],
        "comments": [
            {
                "author": "Eve Legacy",
                "created": "2024-06-01T11:00:00.000+0000",
                "body": (
                    "h1. Project space is not a backup\n\n"
                    "Old wording: project dirs are *not* backed up. "
                    "Users should copy important data themselves.\n\n"
                    "Cheers,\nEve\n"  # personal + stale
                ),
            }
        ],
    },
]


def _auth_ok(handler: BaseHTTPRequestHandler) -> bool:
    header = handler.headers.get("Authorization", "")
    if header.startswith("Basic "):
        try:
            raw = base64.b64decode(header[6:]).decode("utf-8")
        except Exception:
            return False
        user, _, pw = raw.partition(":")
        return user == USER and pw == PASSWORD
    if header.startswith("Bearer "):
        return header[7:].strip() == PAT
    # Jira Cloud-style basic with email:token still maps to Basic.
    return False


def _jql_match(issue: dict, jql: str) -> bool:
    """Tiny JQL subset: project = KEY, labels = name, AND only."""
    jql_l = jql.strip()
    # project = HSP
    m = re.search(r"project\s*=\s*(\w+)", jql_l, re.I)
    if m:
        prefix = m.group(1).upper() + "-"
        if not issue["key"].startswith(prefix):
            return False
    # labels = canned-response  (single equality only)
    for lab in re.findall(r"labels\s*=\s*([\w-]+)", jql_l, re.I):
        if lab not in issue["labels"]:
            return False
    # labels in (a, b) — optional
    m_in = re.search(r"labels\s+in\s*\(([^)]+)\)", jql_l, re.I)
    if m_in:
        wanted = {x.strip().strip("\"'") for x in m_in.group(1).split(",")}
        if not wanted.intersection(issue["labels"]):
            return False
    return True


class Handler(BaseHTTPRequestHandler):
    server_version = "canonic-jira-fixture/1.0"

    def log_message(self, fmt: str, *args) -> None:
        print(f"[jira-fixture] {self.address_string()} - {fmt % args}")

    def _json(self, code: int, payload: object) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _unauth(self) -> None:
        self.send_response(401)
        self.send_header("WWW-Authenticate", 'Basic realm="jira-fixture"')
        self.end_headers()

    def do_GET(self) -> None:  # noqa: N802
        parsed = urlparse(self.path)
        path = parsed.path.rstrip("/") or "/"
        qs = parse_qs(parsed.query)

        # Liveness without credentials (container health probes).
        if path in ("/status", "/health", "/"):
            self._json(
                200,
                {
                    "status": "ok",
                    "fixture": "canonic-jira-fixture",
                    "issues": len(ISSUES),
                },
            )
            return

        if not _auth_ok(self):
            self._unauth()
            return

        if path == "/rest/api/2/search" or path == "/rest/api/2/search/":
            jql = qs.get("jql", [""])[0]
            start = int(qs.get("startAt", ["0"])[0])
            max_r = int(qs.get("maxResults", ["50"])[0])
            matched = [i for i in ISSUES if _jql_match(i, jql)]
            page = matched[start : start + max_r]
            payload = {
                "expand": "names,schema",
                "startAt": start,
                "maxResults": max_r,
                "total": len(matched),
                "issues": [
                    {
                        "id": str(10000 + n),
                        "key": i["key"],
                        "self": f"http://localhost:{PORT}/rest/api/2/issue/{10000 + n}",
                        "fields": {"summary": i["summary"]},
                    }
                    for n, i in enumerate(page, start=start)
                ],
            }
            self._json(200, payload)
            return

        m = re.fullmatch(r"/rest/api/2/issue/([A-Z]+-\d+)/comment", path)
        if m:
            key = m.group(1)
            issue = next((i for i in ISSUES if i["key"] == key), None)
            if issue is None:
                self._json(404, {"errorMessages": [f"Issue Does Not Exist: {key}"]})
                return
            comments = []
            for n, c in enumerate(issue["comments"]):
                comments.append(
                    {
                        "id": str(20000 + n),
                        "author": {
                            "name": c["author"].split()[0].lower(),
                            "displayName": c["author"],
                        },
                        "body": c["body"],
                        "created": c["created"],
                    }
                )
            self._json(
                200,
                {
                    "startAt": 0,
                    "maxResults": len(comments),
                    "total": len(comments),
                    "comments": comments,
                },
            )
            return

        self._json(404, {"errorMessages": [f"no fixture route for {path}"]})


def main() -> None:
    httpd = ThreadingHTTPServer((HOST, PORT), Handler)
    print(
        f"[jira-fixture] listening on http://{HOST}:{PORT} "
        f"(basic {USER}:{PASSWORD} or Bearer {PAT})"
    )
    httpd.serve_forever()


if __name__ == "__main__":
    main()
