Usage
=====

.. raw:: html

   <div class="cn-page-intro">
     <p>Day-to-day workflow for the shared <code>resp-</code> corpus: scaffold and
     edit responses, quality-gate in CI, browse with the TUI, convert with
     pandoc, and use free-tier Jira REST only for explicit probe / import /
     one-shot comment — never unattended bulk sync into Jira.</p>
   </div>

Quick start
-----------

.. code:: shell

   cargo install --git https://github.com/HaoZeke/canonic --locked
   canonic doctor
   canonic list
   canonic tui                          # interactive browser
   canonic check && canonic lint --engine harper

From a checkout of this repo (corpus sample included):

.. code:: shell

   cargo build --release
   ./target/release/canonic list
   ./target/release/canonic tui

Operational loop
----------------

.. raw:: html

   <ol class="cn-flow">
     <li>
       <span class="cn-flow-n">1</span>
       <span class="cn-flow-title">Scaffold or import</span>
       <span class="cn-flow-desc"><code>new</code> for a template, or <code>import-jira</code> into <code>corpus/imports/</code>.</span>
     </li>
     <li>
       <span class="cn-flow-n">2</span>
       <span class="cn-flow-title">Edit &amp; gate</span>
       <span class="cn-flow-desc"><code>check</code> + Harper lint; promote only when clean.</span>
     </li>
     <li>
       <span class="cn-flow-n">3</span>
       <span class="cn-flow-title">Browse &amp; search</span>
       <span class="cn-flow-desc">TUI or CLI list/search/dedupe before publishing.</span>
     </li>
     <li>
       <span class="cn-flow-n">4</span>
       <span class="cn-flow-title">Render / post</span>
       <span class="cn-flow-desc"><code>convert</code> for wiki paste, or explicit <code>jira-comment</code> (one file → one issue).</span>
     </li>
   </ol>

Command reference
-----------------

.. list-table::
   :header-rows: 1
   :widths: 22 38 40

   * - Command
     - Purpose
     - Notes
   * - ``doctor``
     - Probe pandoc, Vale, Harper, optional Jira env
     - Exit ``1`` if pandoc missing
   * - ``tui``
     - Interactive corpus browser
     - See :ref:`tui-section`
   * - ``list``
     - List published responses
     - ``--corpus DIR``
   * - ``new "Title"``
     - Scaffold check-clean ``resp-`` markdown
     - ``--id``, ``--tags``, ``--sop``, ``--out``, ``--force``
   * - ``promote PATH``
     - Copy import draft → ``corpus/responses/``
     - Runs quality check first; refuses dirty drafts
   * - ``check``
     - Quality gate (prefix, sop, team sign-off)
     - Exit ``1`` on findings; ``--json``
   * - ``lint``
     - Vale and/or Harper
     - ``--engine harper`` for CI (in-process)
   * - ``convert [PATH]``
     - Markdown → Jira wiki via pandoc
     - Whole corpus if PATH omitted; ``--write``
   * - ``reindex``
     - Rebuild Tantivy index
     - Default ``.canonic-index/``
   * - ``search QUERY``
     - BM25 full-text search
     - Requires prior ``reindex``
   * - ``dedupe``
     - Near-duplicate pairs
     - ``--threshold``, ``--reindex``, ``--json``
   * - ``jira-probe``
     - Free REST identity (``/myself``)
     - Env auth; no Marketplace apps
   * - ``import-jira JQL``
     - Pull comments as review drafts
     - Writes ``corpus/imports/`` only
   * - ``jira-comment``
     - Post one pandoc-jira comment
     - Explicit, not bulk; ``--dry-run``

Command map (cards)
-------------------

.. grid:: 1 2 2 2
   :gutter: 2

   .. grid-item-card:: Environment &amp; corpus
      :class-card: sd-border-0

      .. raw:: html

         <ul class="cn-cmd-list">
           <li>canonic doctor<span class="cn-cmd-hint">pandoc / tooling status</span></li>
           <li>canonic tui<span class="cn-cmd-hint">interactive browser</span></li>
           <li>canonic list<span class="cn-cmd-hint">list response files</span></li>
           <li>canonic check<span class="cn-cmd-hint">quality gate · exit 1 on findings</span></li>
           <li>canonic lint --engine harper</li>
         </ul>

   .. grid-item-card:: Author &amp; promote
      :class-card: sd-border-0

      .. raw:: html

         <ul class="cn-cmd-list">
           <li>canonic new "Title"<span class="cn-cmd-hint">scaffold resp- template</span></li>
           <li>canonic promote PATH.md<span class="cn-cmd-hint">import → responses after check</span></li>
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

   .. grid-item-card:: Convert &amp; free Jira
      :class-card: sd-border-0

      .. raw:: html

         <ul class="cn-cmd-list">
           <li>canonic convert PATH.md<span class="cn-cmd-hint">pandoc jira writer</span></li>
           <li>canonic jira-probe<span class="cn-cmd-hint">myself · free platform only</span></li>
           <li>canonic import-jira "JQL"<span class="cn-cmd-hint">drafts under corpus/imports/</span></li>
           <li>canonic jira-comment --issue KEY PATH.md<span class="cn-cmd-hint">explicit POST comment</span></li>
         </ul>

.. _tui-section:

Interactive TUI
---------------

``canonic tui`` opens a **ratatui** corpus browser over ``corpus/responses/``
(override with ``--corpus``).

+----------+-----------------------------------------------+
| Key      | Action                                        |
+==========+===============================================+
| ``j``/↓  | Next response                                 |
+----------+-----------------------------------------------+
| ``k``/↑  | Previous response                             |
+----------+-----------------------------------------------+
| ``/``    | Filter by id, title, tags, or body            |
+----------+-----------------------------------------------+
| ``C``    | Run quality check on the whole corpus         |
+----------+-----------------------------------------------+
| ``c``    | Convert selection → jira wiki (preview only)  |
+----------+-----------------------------------------------+
| ``l``    | Lint selection with in-process Harper         |
+----------+-----------------------------------------------+
| ``r``    | Rebuild Tantivy index                         |
+----------+-----------------------------------------------+
| ``s``    | Search index (filter text or selected title)  |
+----------+-----------------------------------------------+
| ``R``    | Reload markdown from disk                     |
+----------+-----------------------------------------------+
| ``d``    | Doctor / tooling snapshot in preview          |
+----------+-----------------------------------------------+
| ``?``    | Help overlay                                  |
+----------+-----------------------------------------------+
| ``q``    | Quit                                          |
+----------+-----------------------------------------------+

.. important::

   The TUI **never** POSTs to Jira. Convert preview stays local. Publishing is
   always an explicit ``canonic jira-comment`` (or human paste of ``convert``
   output).

Recipes
-------

**New response from scratch**

.. code:: shell

   canonic new "Project space is not a backup" --tags storage,project-space
   $EDITOR corpus/responses/resp-project-space-is-not-a-backup.md
   canonic check
   canonic lint --engine harper
   canonic tui

**Import → edit → promote**

.. code:: shell

   export JIRA_BASE_URL=https://your-instance.atlassian.net
   export JIRA_EMAIL=you@example.org JIRA_API_TOKEN=...
   canonic jira-probe
   canonic import-jira "project = HSP AND labels = canned-response" --dry-run
   canonic import-jira "project = HSP AND labels = canned-response"
   $EDITOR corpus/imports/resp-….md    # team sign-off, shared voice
   canonic promote corpus/imports/resp-….md
   canonic check

**Search before answering a ticket**

.. code:: shell

   canonic reindex
   canonic search "project space backup"
   canonic dedupe --reindex --threshold 1.0
   canonic tui    # / to filter, c to preview jira markup

**Publish one comment (human-gated)**

.. code:: shell

   canonic convert corpus/responses/resp-project-space-is-not-a-backup.md
   canonic jira-comment --issue HSP-101 \
     corpus/responses/resp-project-space-is-not-a-backup.md --dry-run
   canonic jira-comment --issue HSP-101 \
     corpus/responses/resp-project-space-is-not-a-backup.md

Full paste-ready session
------------------------

.. code:: shell

   canonic doctor
   canonic list
   canonic new "Example topic" --tags example
   canonic check
   canonic lint --engine harper
   canonic convert corpus/responses/resp-project-space-is-not-a-backup.md
   canonic tui

   canonic reindex
   canonic search "project space backup"
   canonic dedupe --reindex --threshold 1.0

   JIRA_BASE_URL=https://your-instance.atlassian.net \
   JIRA_EMAIL=you@example.org JIRA_API_TOKEN=... \
     canonic jira-probe
   canonic import-jira "project = HSP AND labels = canned-response" --dry-run
   canonic promote corpus/imports/resp-example-hsp-101.md
   canonic jira-comment --issue HSP-101 \
     corpus/responses/resp-project-space-is-not-a-backup.md --dry-run

Corpus layout
-------------

Responses live under ``corpus/responses/`` as ``resp-<topic-slug>.md``.
Import drafts land under ``corpus/imports/`` (gitignored) until ``promote``.
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
   - ``sop:`` required — Confluence / service-desk wiki URL, or the literal
     ``none`` when no SOP page exists yet
   - Closings must be team-generic (e.g. ``Support Team``), not personal names

``.gitignore`` excludes the Tantivy index under ``.canonic-index/`` and
review drafts under ``corpus/imports/`` (never commit un-promoted imports).

Team review via GitLab mirror
-----------------------------

For team merge-request review on a GitLab remote while the primary clone is
elsewhere, mirror a branch:

.. code:: shell

   export CANONIC_GITLAB_REMOTE=git@gitlab.example.com:your-group/canonic.git
   scripts/mirror-to-gitlab.sh

Open the merge request on that GitLab. Do **not** bulk-push the library into
Jira; only ``convert`` paste or explicit ``jira-comment`` publish one answer.

CI quality gate
---------------

GitHub Actions runs ``cargo test --locked --all-targets``, then on the release
binary:

1. ``canonic list`` / ``canonic check`` on ``corpus/responses/``
2. ``canonic lint --engine harper`` (in-process; domain vocab for the cluster/HPC)
3. ``canonic convert`` smoke on the seeded sample (pandoc installed in CI)

Keep published responses check-clean so the gate stays green.

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
available). Exit non-zero on auth or reachability failure.

**Import (read path)**

``import-jira <jql>`` searches free search paths (api/2 then api/3 variants),
fetches comments, converts wiki/ADF → markdown, and writes **one draft per
issue** under ``corpus/imports/`` — never into ``corpus/responses/``.
``--dry-run`` lists targets without fetching comments.

**Comment write (explicit)**

``jira-comment --issue KEY PATH.md`` converts markdown with pandoc's free
``jira`` writer, then posts via platform REST:

- Server/DC: ``POST /rest/api/2/issue/{key}/comment`` with wiki string body
- Cloud Free (``*.atlassian.net``): ``POST /rest/api/3/issue/{key}/comment`` with
  minimal **ADF** — required by Cloud v3, no paid apps

``--body-format auto|wiki|adf`` overrides host detection. ``--dry-run`` prints the
converted wiki without POSTing. One file, one issue, human-gated — not bulk sync.

**Official free map (platform only)**

+------------------+----------------------------------------------+------------------+
| Command          | Endpoint                                     | Body             |
+==================+==============================================+==================+
| ``jira-probe``   | ``GET /rest/api/2/myself``                   | —                |
+------------------+----------------------------------------------+------------------+
| ``import-jira``  | ``GET …/search`` + comments (v2/v3 fallback) | wiki or ADF read |
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

Agent skill
-----------

Day-to-day agent instructions ship at
``.agents/skills/canonic-canned-loop/SKILL.md`` (import → scaffold/promote →
check/lint → TUI → convert / ``jira-comment``). Install or trust that path in
your agent skill loader.

Exit codes
----------

+-------------+--------------------------------------------------+
| Command     | Non-zero meaning                                 |
+=============+==================================================+
| ``doctor``  | ``1`` if pandoc missing (convert blocked)        |
+-------------+--------------------------------------------------+
| ``check``   | ``1`` if any quality finding                     |
+-------------+--------------------------------------------------+
| ``lint``    | ``1`` if any lint finding                        |
+-------------+--------------------------------------------------+
| ``promote`` | check failed or destination exists without force |
+-------------+--------------------------------------------------+

GitLab mirror
-------------

``scripts/mirror-to-gitlab.sh`` pushes the current branch to a second remote for
team merge-request review on self-hosted GitLab:

.. code:: shell

   CANONIC_GITLAB_REMOTE=git@gitlab.example:group/canonic.git scripts/mirror-to-gitlab.sh

Pass a branch name as the first argument to override the current branch.
