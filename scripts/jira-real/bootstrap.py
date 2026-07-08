#!/usr/bin/env python3
"""One-shot classic setup + seed for Atlassian Jira Software 9.x."""
from __future__ import annotations

import base64
import json
import re
import sys
import time
import urllib.error
import urllib.request
import http.cookiejar
from urllib.parse import urlencode

BASE = "http://127.0.0.1:18080"
# Public Atlassian 3-hour Jira Software DC timebomb (Marketplace developer docs)
LICENSE = (
    "AAAB8w0ODAoPeNp9Uk2P2jAQvedXWOoNydmELVKLFKlL4u7SLglKQj+27cEkA3gb7GjssMu/"
    "rwnQls9DDvHMvPfmvXmTN0BGfE08n3jdftfv927J/SgnXc9/58wRQC5UXQO6j6IAqYGVwggl"
    "AxbnLB2nw4w5cbOcAiaziQbUge85oZKGFybmSwjKmiMKvfjATcW1Fly6hVo64waLBdcQcQPB"
    "hot6Per5zo4lX9fQjofJaMTScHj3uC+x11rgup0b3z7sudiIi+oSWQa4AhxGweD+fU6/Tb68"
    "pZ+fnh7owPO/Os8CuVujKpvCuJsfqtXMvHAE1+KKFQQGG3A+2cp412XJeQjSHLVkzVQXKOrW"
    "n/bljH/nNmslXPa30+nESU4/Jikdp0k0CfNhEtNJxmwhCBGsFSWZrolZANmhECYLVQISu9gz"
    "FIb8WBhT/+zf3MyVe2DOTbWdoLCd+OWSSBGpDCmFNiimjQGLLDQxihSNNmppU3Yd67c0ILks"
    "jhOxqsKU3eUsooPvG4kXUrli/MlF7dayEU7kb6lepJOxOLAf7XneFmkfCuCp95nh+Ldwhfeg"
    "L8E5l0LzNo4IVlApi0Vy0GZvs9O6b+vHZxzBv0toB3Yuk5lCwuualHs8fSD0/3NqdZ48nBd+"
    "5bjYilfNdokZr6zmP7TmY5YwLAIUNq8MbmR8GfaV9ulfLz1K+3g9j1YCFDeq7aYROMQbwMIv"
    "HimNt7/bJCCIX02nj"
)
# Disposable fixture-only admin for the local Jira Software smoke (not production).
ADMIN_USER = "admin"
ADMIN_PASS = "CanonicAdmin!2026"
ADMIN_EMAIL = "admin@example.com"
ADMIN_FULL = "Canonic Admin"

cj = http.cookiejar.CookieJar()
opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cj))


def get(path, timeout=180):
    with opener.open(BASE + path, timeout=timeout) as r:
        return r.read().decode("utf-8", "replace"), r.geturl()


def post_form(path, data, timeout=900):
    req = urllib.request.Request(BASE + path, data=urlencode(data).encode(), method="POST")
    req.add_header("Content-Type", "application/x-www-form-urlencoded")
    # Some Jira versions want these
    req.add_header("X-Atlassian-Token", "no-check")
    try:
        with opener.open(req, timeout=timeout) as r:
            return r.read().decode("utf-8", "replace"), r.geturl(), r.status
    except urllib.error.HTTPError as e:
        return e.read().decode("utf-8", "replace"), getattr(e, "url", path), e.code


def title(html):
    m = re.search(r"<title>([^<]+)", html)
    return m.group(1).strip() if m else "?"


def token(html):
    m = re.search(r'name="atl_token"[\s\S]{0,200}?value="([^"]+)"', html)
    if not m:
        m = re.search(r'value="([^"]+)"[\s\S]{0,200}?name="atl_token"', html)
    if not m:
        m = re.search(r'name="atlassian-token" content="([^"]+)"', html)
    if not m:
        raise RuntimeError("no token on " + title(html))
    return m.group(1)


def log(msg, html="", url=""):
    print(f"{msg} | {title(html) if html else ''} @ {url}", flush=True)


def wait_http(path="/secure/SetupMode!default.jspa", tries=120):
    for i in range(tries):
        try:
            html, url = get(path)
            if "Jira" in title(html) or "setup" in title(html).lower():
                return html, url
        except Exception as e:
            print("wait", i, e, flush=True)
        time.sleep(3)
    raise SystemExit("never up")


def rest(method, path, payload=None, auth=True, timeout=120):
    data = None if payload is None else json.dumps(payload).encode()
    req = urllib.request.Request(BASE + path, data=data, method=method)
    req.add_header("Accept", "application/json")
    if payload is not None:
        req.add_header("Content-Type", "application/json")
    if auth:
        b = base64.b64encode(f"{ADMIN_USER}:{ADMIN_PASS}".encode()).decode()
        req.add_header("Authorization", f"Basic {b}")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            body = r.read().decode()
            return r.status, (json.loads(body) if body else {})
    except urllib.error.HTTPError as e:
        body = e.read().decode()
        try:
            parsed = json.loads(body)
        except Exception:
            parsed = {"raw": body[:800]}
        return e.code, parsed


def main():
    print("wait setup mode")
    html, url = wait_http()
    log("got", html, url)

    # Mode
    if "setupOption" in html or "SetupMode" in url:
        html, url, st = post_form(
            "/secure/SetupMode.jspa",
            {"setupOption": "classic", "atl_token": token(html)},
        )
        log(f"mode {st}", html, url)

    # DB
    if "databaseOption" in html or "SetupDatabase" in url:
        html, url, st = post_form(
            "/secure/SetupDatabase.jspa",
            {
                "databaseOption": "internal",
                "testingConnection": "false",
                "atl_token": token(html),
            },
            timeout=900,
        )
        log(f"db {st}", html, url)

    # Props
    if 'name="title"' in html or "SetupApplicationProperties" in url:
        html, url, st = post_form(
            "/secure/SetupApplicationProperties.jspa",
            {
                "title": "canonic-smoke",
                "mode": "private",
                "baseURL": BASE,
                "nextStep": "true",
                "atl_token": token(html),
            },
        )
        log(f"props {st}", html, url)

    # License
    if "SetupLicense" not in url:
        html, url = get("/secure/SetupLicense!default.jspa")
        log("license page", html, url)
    html, url, st = post_form(
        "/secure/SetupLicense.jspa",
        {
            "setupLicenseKey": LICENSE,
            "atl_token": token(html),
            "next": "Next",
        },
        timeout=300,
    )
    log(f"license {st}", html, url)
    open("/tmp/jira-oneshot-license.html", "w").write(html)
    if "SetupAdmin" not in url and "username" not in html:
        print(html[html.lower().find("error") : html.lower().find("error") + 300] if "error" in html.lower() else "no admin redirect")
        # still try admin page after plugin restart
        time.sleep(15)

    # After license Jira restarts plugins — wait for admin page
    for i in range(40):
        try:
            html, url = get("/secure/SetupAdminAccount!default.jspa")
            if "username" in html and "atl_token" in html:
                log(f"admin page ready {i}", html, url)
                break
        except Exception as e:
            print("admin wait", i, e, flush=True)
        time.sleep(3)
    else:
        raise SystemExit("admin page never ready")

    html, url, st = post_form(
        "/secure/SetupAdminAccount.jspa",
        {
            "fullname": ADMIN_FULL,
            "email": ADMIN_EMAIL,
            "username": ADMIN_USER,
            "password": ADMIN_PASS,
            "confirm": ADMIN_PASS,
            "atl_token": token(html),
            "next": "Next",
        },
    )
    log(f"admin {st}", html, url)
    open("/tmp/jira-oneshot-admin.html", "w").write(html)
    if st == 403:
        raise SystemExit("admin 403 — XSRF or session broken")

    # Mail (required to complete setup; no SMTP)
    for attempt in range(5):
        try:
            html, url = get("/secure/SetupMailNotifications!default.jspa")
        except Exception as e:
            print("mail get", e)
            break
        if "already completed" in title(html).lower() or "Dashboard" in url:
            log("setup already complete", html, url)
            break
        if "noemail" not in html and "SetupMail" not in url:
            break
        try:
            html, url, st = post_form(
                "/secure/SetupMailNotifications.jspa",
                {
                    "atl_token": token(html),
                    "noemail": "true",
                    "finish": "Finish",
                },
            )
            log(f"mail {st}", html, url)
            if "already completed" in title(html).lower() or "SetupMail" not in url:
                break
        except Exception as e:
            print("mail", e)
            break

    # Wait REST
    for i in range(60):
        code, body = rest("GET", "/rest/api/2/serverInfo", auth=True)
        print("serverInfo", i, code, str(body)[:180], flush=True)
        if code == 200:
            break
        time.sleep(3)
    else:
        raise SystemExit("REST dead")

    code, me = rest("GET", "/rest/api/2/myself")
    print("myself", code, me)
    if code != 200:
        raise SystemExit("auth fail")

    # Project + issues
    code, projects = rest("GET", "/rest/api/2/project")
    keys = {p.get("key") for p in (projects if isinstance(projects, list) else [])}
    if "HSP" not in keys:
        for payload in [
            {
                "key": "HSP",
                "name": "HPC Support",
                "projectTypeKey": "software",
                "projectTemplateKey": "com.pyxis.greenhopper.jira:gh-simplified-basic",
                "lead": ADMIN_USER,
            },
            {"key": "HSP", "name": "HPC Support", "projectTypeKey": "business", "lead": ADMIN_USER},
            {"key": "HSP", "name": "HPC Support", "projectTypeKey": "software", "lead": ADMIN_USER},
        ]:
            code, body = rest("POST", "/rest/api/2/project", payload)
            print("project", code, body)
            if code in (200, 201):
                break
        else:
            raise SystemExit("project fail")

    issues = [
        (
            "Project space is not a backup",
            ["canned-response", "storage"],
            [
                "h1. Project space is not a backup\n\nDemo cluster *project space* is for active working data, not long-term archival. Use the self-service backup options described in the storage SOP.\n\nRegards,\nAlice Advisor\n",
                "Also remind users that {{/home}} and scratch are separate quotas. See the *storage* FAQ.\n",
            ],
        ),
        (
            "How to request a software install on Demo cluster",
            ["canned-response", "software"],
            [
                "h2. Software install requests\n\nPlease open a ticket with:\n* package name and version\n* why the central module stack is insufficient\n* license constraints if any\n\nWe prefer EasyBuild easyconfigs when available.\n\nRegards,\nSupport Team\n"
            ],
        ),
        ("Unrelated networking question", ["networking"], ["Please use eduVPN for off-site access.\n"]),
        (
            "Project space backup policy (stale duplicate)",
            ["canned-response", "storage"],
            [
                "h1. Project space is not a backup\n\nOld wording: project dirs are *not* backed up. Users should copy important data themselves.\n\nCheers,\nEve\n"
            ],
        ),
    ]
    created = []
    for summary, labels, comments in issues:
        code, body = rest(
            "POST",
            "/rest/api/2/issue",
            {
                "fields": {
                    "project": {"key": "HSP"},
                    "summary": summary,
                    "issuetype": {"name": "Task"},
                    "labels": labels,
                }
            },
        )
        print("issue", summary[:40], code, body if code >= 300 else body.get("key"))
        if code not in (200, 201):
            for it in ("Bug", "Story", "New Feature", "Improvement"):
                code, body = rest(
                    "POST",
                    "/rest/api/2/issue",
                    {
                        "fields": {
                            "project": {"key": "HSP"},
                            "summary": summary,
                            "issuetype": {"name": it},
                            "labels": labels,
                        }
                    },
                )
                if code in (200, 201):
                    break
            print(" retry", code, body if code >= 300 else body.get("key"))
        if code not in (200, 201):
            raise SystemExit("issue fail " + summary)
        key = body["key"]
        created.append(key)
        for c in comments:
            cc, _ = rest("POST", f"/rest/api/2/issue/{key}/comment", {"body": c})
            print("  comment", key, cc)

    jql = "project = HSP AND labels = canned-response"
    code, body = rest(
        "GET",
        "/rest/api/2/search?" + urlencode({"jql": jql, "fields": "summary,labels"}),
    )
    print("SEARCH", code, json.dumps(body)[:1500])
    print("CREATED", created)
    print("ONESHOT_OK")


if __name__ == "__main__":
    main()
