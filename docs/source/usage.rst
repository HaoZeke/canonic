Usage
=====

CLI overview
------------

.. code:: shell

   canonic doctor
   canonic list
   canonic check                          # quality gate (exit 1 on findings)
   canonic convert corpus/responses/resp-example-topic.md
   canonic lint --engine harper

   canonic reindex
   canonic search "project space backup"
   canonic dedupe --reindex --threshold 1.0
   canonic dedupe --threshold 0.5 --json

   JIRA_BASE_URL=https://your-instance.atlassian.net JIRA_EMAIL=you@example.org JIRA_API_TOKEN=... \
     canonic import-jira "project = HSP AND labels = canned-response" --dry-run

Corpus layout
-------------

Responses live under ``corpus/responses/`` as ``resp-<topic-slug>.md``.
Front matter (enforced by ``canonic check``):

.. code:: markdown

   ---
   id: resp-<topic-slug>
   title: Human-readable title
   prefix: resp
   tags: [tag-one, tag-two]
   sop: none
   ---

- ``id`` and filename stem must match and start with ``resp-``
- ``prefix: resp`` required (shared advisor library; no personal prefixes)
- ``sop:`` required — Confluence URL or literal ``none``
- Closings must be team-generic (e.g. ``Support Team``), not personal names

``.gitignore`` excludes the Tantivy index under ``.canonic-index/``.

Dedupe
------

``dedupe`` rebuilds or reuses the Tantivy index, then for each response runs a
self-query (title + content terms) and reports other documents that rank above
``--threshold``. Pair reasons include the Tantivy score and a content **Jaccard**
similarity. Use a high threshold to list only strong near-copies when curating
the library before a Jira migration.

Jira import (read path)
-----------------------

``import-jira <jql>`` searches Jira via REST API v2, converts each comment's wiki
markup back to markdown with pandoc, and writes one draft file per issue under
``corpus/imports/`` — never directly into ``corpus/responses/``. ``--dry-run``
lists which issues would be imported without fetching comments.

Authentication (environment):

- ``JIRA_BASE_URL`` — required
- ``JIRA_EMAIL`` + ``JIRA_API_TOKEN`` — Basic auth (Jira Cloud)
- ``JIRA_AUTH_HEADER`` — raw ``Authorization`` header (takes precedence)

Exit codes
----------

- ``doctor``: ``1`` if pandoc missing (convert blocked)
- ``check``: ``1`` if any quality finding

GitLab mirror
-------------

``scripts/mirror-to-gitlab.sh`` pushes the current branch to a second remote for
team merge-request review on self-hosted GitLab:

.. code:: shell

   CANONIC_GITLAB_REMOTE=git@gitlab.example:group/canonic.git scripts/mirror-to-gitlab.sh
