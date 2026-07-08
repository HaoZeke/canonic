Usage
=====

.. raw:: html

   <div class="cn-page-intro">
     <p>Day-to-day CLI for the shared <code>resp-</code> corpus: quality gates, search,
     convert, and a read-only Jira import path that never writes into
     <code>corpus/responses/</code> unattended.</p>
   </div>

Command map
-----------

.. grid:: 1 2 2 2
   :gutter: 2

   .. grid-item-card:: Environment &amp; corpus
      :class-card: sd-border-0

      .. raw:: html

         <ul class="cn-cmd-list">
           <li>canonic doctor<span class="cn-cmd-hint">pandoc / tooling status</span></li>
           <li>canonic list<span class="cn-cmd-hint">list response files</span></li>
           <li>canonic check<span class="cn-cmd-hint">quality gate · exit 1 on findings</span></li>
           <li>canonic lint --engine harper</li>
         </ul>

   .. grid-item-card:: Search &amp; dedupe
      :class-card: sd-border-0

      .. raw:: html

         <ul class="cn-cmd-list">
           <li>canonic reindex</li>
           <li>canonic search "project space backup"</li>
           <li>canonic dedupe --reindex --threshold 1.0</li>
           <li>canonic dedupe --threshold 0.5 --json</li>
         </ul>

   .. grid-item-card:: Convert &amp; publish
      :class-card: sd-border-0

      .. raw:: html

         <ul class="cn-cmd-list">
           <li>canonic convert corpus/responses/resp-….md<span class="cn-cmd-hint">needs pandoc jira writer</span></li>
         </ul>

   .. grid-item-card:: Free Jira REST
      :class-card: sd-border-0

      .. raw:: html

         <ul class="cn-cmd-list">
           <li>canonic jira-probe<span class="cn-cmd-hint">myself · free platform only</span></li>
           <li>canonic import-jira "JQL" --dry-run</li>
           <li>canonic import-jira "JQL"<span class="cn-cmd-hint">drafts under corpus/imports/</span></li>
           <li>canonic jira-comment --issue KEY PATH.md<span class="cn-cmd-hint">explicit POST comment</span></li>
         </ul>

Full paste-ready session
------------------------

.. code:: shell

   canonic doctor
   canonic list
   canonic check
   canonic convert corpus/responses/resp-example-topic.md
   canonic lint --engine harper

   canonic reindex
   canonic search "project space backup"
   canonic dedupe --reindex --threshold 1.0
   canonic dedupe --threshold 0.5 --json

   JIRA_BASE_URL=https://your-instance.atlassian.net \
   JIRA_EMAIL=you@example.org JIRA_API_TOKEN=... \
     canonic jira-probe
   canonic import-jira "project = HSP AND labels = canned-response" --dry-run
   canonic jira-comment --issue HSP-101 corpus/responses/resp-example-topic.md --dry-run

Corpus layout
-------------

Responses live under ``corpus/responses/`` as ``resp-<topic-slug>.md``.
Front matter is enforced by ``canonic check``:

.. code:: markdown

   ---
   id: resp-<topic-slug>
   title: Human-readable title
   prefix: resp
   tags: [tag-one, tag-two]
   sop: none
   ---

.. important::

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
similarity for a second opinion.

.. tip::

   Use a **high** threshold when you only want strong near-copies (curation
   before a Jira migration). Drop the threshold and add ``--json`` when you
   want a wider review list.

Free Jira REST (no paid Marketplace apps)
-----------------------------------------

canonic uses only **native platform REST** (Cloud Free API tokens or Server/DC
PAT). No Marketplace extensions, ScriptRunner, or paid Service Desk canned-response
admin APIs.

**Probe**

``canonic jira-probe`` calls ``GET /rest/api/2/myself`` (and serverInfo when
available) using the env auth below. Exit non-zero on auth or reachability failure.

**Import (read path)**

``import-jira <jql>`` searches via ``GET /rest/api/2/search``, fetches comments,
converts wiki → markdown with pandoc, and writes **one draft per issue** under
``corpus/imports/`` — never into ``corpus/responses/``. ``--dry-run`` lists
targets without fetching comments.

**Comment write (explicit)**

``jira-comment --issue KEY PATH.md`` converts markdown with pandoc's free
``jira`` writer, then posts via platform REST:

- Server/DC: ``POST /rest/api/2/issue/{key}/comment`` with wiki string body
- Cloud Free (``*.atlassian.net``): ``POST /rest/api/3/issue/{key}/comment`` with
  minimal **ADF** (Atlassian Document Format) — required by Cloud v3, no paid apps

``--body-format auto|wiki|adf`` overrides host detection. ``--dry-run`` prints the
converted wiki without POSTing. One file, one issue, human-gated — not bulk sync.

**Official free map (platform only)**

+------------------+----------------------------------------------+------------------+
| Command          | Endpoint                                     | Body             |
+==================+==============================================+==================+
| ``jira-probe``   | ``GET /rest/api/2/myself``                   | —                |
+------------------+----------------------------------------------+------------------+
| ``import-jira``  | ``GET /rest/api/2/search`` + comments        | wiki or ADF read |
+------------------+----------------------------------------------+------------------+
| ``jira-comment`` | ``POST …/comment`` (v2 wiki / v3 ADF)        | pandoc jira text |
+------------------+----------------------------------------------+------------------+

**Authentication (environment):**

+------------------------+--------------------------------------------------+
| Variable               | Role                                             |
+========================+==================================================+
| ``JIRA_BASE_URL``      | Required, e.g. Cloud Free instance URL           |
+------------------------+--------------------------------------------------+
| ``JIRA_EMAIL`` +       | Basic auth (Jira Cloud Free API token)           |
| ``JIRA_API_TOKEN``     |                                                  |
+------------------------+--------------------------------------------------+
| ``JIRA_AUTH_HEADER``   | Raw ``Authorization`` header; wins if set        |
|                        | (e.g. ``Bearer …`` for Server/DC)                |
+------------------------+--------------------------------------------------+

Exit codes
----------

+-------------+--------------------------------------------------+
| Command     | Non-zero meaning                                 |
+=============+==================================================+
| ``doctor``  | ``1`` if pandoc missing (convert blocked)        |
+-------------+--------------------------------------------------+
| ``check``   | ``1`` if any quality finding                     |
+-------------+--------------------------------------------------+

GitLab mirror
-------------

``scripts/mirror-to-gitlab.sh`` pushes the current branch to a second remote for
team merge-request review on self-hosted GitLab:

.. code:: shell

   CANONIC_GITLAB_REMOTE=git@gitlab.example:group/canonic.git scripts/mirror-to-gitlab.sh

Pass a branch name as the first argument to override the current branch.
