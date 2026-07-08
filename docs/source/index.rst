.. raw:: html

   <div class="cn-hero">
     <div class="cn-hero-brand">
       <img class="cn-hero-mark" src="_static/mark.svg" width="56" height="56" alt="" />
       <div>
         <p class="cn-hero-name">canonic</p>
       </div>
     </div>
     <img class="cn-hero-logo cn-hero-logo-light" src="_static/logo.svg" width="320" height="60" alt="canonic" />
     <img class="cn-hero-logo cn-hero-logo-dark" src="_static/logo-dark.svg" width="320" height="60" alt="canonic" />
     <p class="cn-hero-tagline">Canonical canned-response corpus for Jira Jira work — markdown under a shared <code>resp-</code> prefix, pandoc convert, quality gates, and Tantivy search / dedupe.</p>
     <div class="cn-hero-pills">
       <span>markdown corpus</span>
       <span>resp- prefix</span>
       <span>pandoc jira</span>
       <span>Tantivy BM25</span>
     </div>
   </div>

Why canonic
===========

Shared advisor answers should live in git, not only in Jira comments.
**canonic** treats version-controlled markdown under ``corpus/responses/`` as the
source of truth: check quality before migration, convert with pandoc's Jira
writer, lint with Vale or in-process Harper, and search or dedupe with a local
Tantivy index.

+----------------------------------+----------------------------------+
| Need                             | Use                              |
+==================================+==================================+
| Validate front matter / closings | ``canonic check``                |
+----------------------------------+----------------------------------+
| Markdown → Jira wiki markup      | ``canonic convert PATH``         |
+----------------------------------+----------------------------------+
| Find similar existing answers    | ``canonic search`` / ``dedupe``  |
+----------------------------------+----------------------------------+
| Pull existing Jira comments      | ``canonic import-jira`` (drafts) |
+----------------------------------+----------------------------------+

Install (shortest path)
=======================

.. code:: shell

   cargo install --git https://github.com/HaoZeke/canonic
   # or from a checkout:
   cargo build --release

Requirements: Rust 1.75+, `pandoc <https://pandoc.org/>`_ for ``convert``,
optional `Vale <https://vale.sh/>`_ for style lint (Harper is linked in-process).

First canned response
=====================

1. Create ``corpus/responses/resp-example-topic.md`` with front matter
   (``id``, ``title``, ``prefix: resp``, ``tags``, ``sop``) and a
   team-generic closing (e.g. ``Support Team``).
2. ``canonic check``
3. ``canonic reindex`` then ``canonic search "example topic"``
4. ``canonic convert corpus/responses/resp-example-topic.md`` (needs pandoc)

Delete the example once you commit a real response under its own id.

Contents
========

.. toctree::
   :maxdepth: 2

   usage
   design
