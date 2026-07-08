#!/usr/bin/env python3
import base64, json, urllib.error, urllib.request
from urllib.parse import urlencode

import os
BASE = os.environ.get("JIRA_BASE_URL", "http://127.0.0.1:18080")
ADMIN_USER = os.environ.get("JIRA_EMAIL", "admin")
ADMIN_PASS = os.environ.get("JIRA_API_TOKEN", "CanonicAdmin!2026")

def rest(method, path, payload=None):
    data = None if payload is None else json.dumps(payload).encode()
    req = urllib.request.Request(BASE + path, data=data, method=method)
    req.add_header("Accept", "application/json")
    if payload is not None:
        req.add_header("Content-Type", "application/json")
    b = base64.b64encode(f"{ADMIN_USER}:{ADMIN_PASS}".encode()).decode()
    req.add_header("Authorization", f"Basic {b}")
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            body = r.read().decode()
            return r.status, json.loads(body) if body else {}
    except urllib.error.HTTPError as e:
        body = e.read().decode()
        try:
            parsed = json.loads(body)
        except Exception:
            parsed = {"raw": body[:800]}
        return e.code, parsed

# list issue types for project
code, meta = rest("GET", "/rest/api/2/issue/createmeta?projectKeys=HSP&expand=projects.issuetypes.fields")
print("createmeta", code)
if code == 200:
    for p in meta.get("projects", []):
        for it in p.get("issuetypes", []):
            fields = it.get("fields", {})
            print(" type", it.get("name"), "labels" in fields, "required", [k for k,v in fields.items() if v.get("required")])

# discover issue types
code, types = rest("GET", "/rest/api/2/issuetype")
print("issuetypes", [(t.get("id"), t.get("name")) for t in (types if isinstance(types, list) else [])])

issues = [
    ("Project space is not a backup", ["canned-response", "storage"], [
        "h1. Project space is not a backup\n\nDemo *project space* is for active working data, not long-term archival. Use the self-service backup options described in the storage SOP.\n\nRegards,\nAlice Advisor\n",
        "Also remind users that {{/home}} and scratch are separate quotas. See the *storage* FAQ.\n",
    ]),
    ("How to request a software install on the cluster", ["canned-response", "software"], [
        "h2. Software install requests\n\nPlease open a ticket with:\n* package name and version\n* why the central module stack is insufficient\n* license constraints if any\n\nWe prefer EasyBuild easyconfigs when available.\n\nRegards,\nSupport Team\n"
    ]),
    ("Unrelated networking question", ["networking"], ["Please use eduVPN for off-site access.\n"]),
    ("Project space backup policy (stale duplicate)", ["canned-response", "storage"], [
        "h1. Project space is not a backup\n\nOld wording: project dirs are *not* backed up. Users should copy important data themselves.\n\nCheers,\nEve\n"
    ]),
]

# pick first non-subtask type
itype = "Task"
if code == 200 and isinstance(types, list):
    for t in types:
        if not t.get("subtask") and t.get("name"):
            itype = t["name"]
            break
print("using issuetype", itype)

# idempotent: if canned-response issues already present, skip create
code, existing = rest("GET", "/rest/api/2/search?" + urlencode({"jql": "project = HSP AND labels = canned-response", "fields": "key"}))
if code == 200 and existing.get("total", 0) >= 3:
    print("already seeded", existing.get("total"), "canned-response issues")
    print("SEED_OK")
    raise SystemExit(0)

created = []
for summary, labels, comments in issues:
    code, body = rest("POST", "/rest/api/2/issue", {
        "fields": {
            "project": {"key": "HSP"},
            "summary": summary,
            "issuetype": {"name": itype},
        }
    })
    print("create", summary[:40], code, body if code >= 300 else body.get("key"))
    if code not in (200, 201):
        # try by id
        for t in (types if isinstance(types, list) else []):
            if t.get("subtask"):
                continue
            code, body = rest("POST", "/rest/api/2/issue", {
                "fields": {
                    "project": {"key": "HSP"},
                    "summary": summary,
                    "issuetype": {"id": t["id"]},
                }
            })
            print("  try type", t["name"], code, body if code >= 300 else body.get("key"))
            if code in (200, 201):
                break
    if code not in (200, 201):
        raise SystemExit("cannot create " + summary)
    key = body["key"]
    created.append(key)
    # set labels via update
    code, body = rest("PUT", f"/rest/api/2/issue/{key}", {
        "fields": {"labels": labels}
    })
    print("  labels", key, labels, code, body if code >= 300 else "ok")
    if code >= 300:
        # alternate edit API
        code, body = rest("PUT", f"/rest/api/2/issue/{key}", {
            "update": {"labels": [{"set": labels}]}
        })
        print("  labels set", code, body if code >= 300 else "ok")
    for c in comments:
        cc, _ = rest("POST", f"/rest/api/2/issue/{key}/comment", {"body": c})
        print("  comment", key, cc)

jql = "project = HSP AND labels = canned-response"
code, body = rest("GET", "/rest/api/2/search?" + urlencode({"jql": jql, "fields": "summary,labels"}))
print("SEARCH", code, json.dumps(body, indent=2)[:2000])
print("CREATED", created)
print("SEED_OK")
