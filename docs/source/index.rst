.. raw:: html

   <div class="cn-hero">
     <p class="cn-hero-eyebrow">markdown corpus · shared advisor library</p>
     <div class="cn-hero-brand">
       <img class="cn-hero-mark" src="_static/mark.svg" width="60" height="60" alt="" />
       <div>
         <p class="cn-hero-name">canonic</p>
         <p class="cn-hero-sub">canonical responses, versioned in git</p>
       </div>
     </div>
     <img class="cn-hero-logo cn-hero-logo-light" src="_static/logo.svg" width="320" height="60" alt="canonic" />
     <img class="cn-hero-logo cn-hero-logo-dark" src="_static/logo-dark.svg" width="320" height="60" alt="canonic" />
     <p class="cn-hero-tagline">Canned-response corpus for Jira Jira work — markdown under a shared <code>resp-</code> prefix is the source of truth. Convert with pandoc, gate with quality checks, search and dedupe with Tantivy.</p>
     <div class="cn-hero-pills">
       <span>markdown corpus</span>
       <span>resp- prefix</span>
       <span>pandoc → jira</span>
       <span>Tantivy BM25</span>
       <span>review before migrate</span>
     </div>
     <div class="cn-hero-cta">
       <a class="cn-btn cn-btn-primary" href="usage.html">Usage &amp; CLI</a>
       <a class="cn-btn cn-btn-ghost" href="design.html">Design notes</a>
       <a class="cn-btn cn-btn-ghost" href="https://github.com/HaoZeke/canonic">GitHub</a>
     </div>
   </div>

Why canonic
===========

Shared advisor answers should live in git, not only in Jira comments.
**canonic** treats version-controlled markdown under ``corpus/responses/`` as the
source of truth: check quality before migration, convert with pandoc's Jira
writer, lint with Vale or in-process Harper, and search or dedupe with a local
Tantivy index.

Workflow at a glance
--------------------

.. raw:: html

   <ol class="cn-flow">
     <li>
       <span class="cn-flow-n">1</span>
       <span class="cn-flow-title">Author in markdown</span>
       <span class="cn-flow-desc">Shared <code>resp-</code> prefix, front matter, team closing.</span>
       <span class="cn-flow-cmd">corpus/responses/</span>
     </li>
     <li>
       <span class="cn-flow-n">2</span>
       <span class="cn-flow-title">Gate quality</span>
       <span class="cn-flow-desc">Ids, prefix, sop, and generic closings — fail the PR early.</span>
       <span class="cn-flow-cmd">canonic check</span>
     </li>
     <li>
       <span class="cn-flow-n">3</span>
       <span class="cn-flow-title">Search &amp; dedupe</span>
       <span class="cn-flow-desc">Local Tantivy BM25 + Jaccard for near-copy discovery.</span>
       <span class="cn-flow-cmd">canonic search · dedupe</span>
     </li>
     <li>
       <span class="cn-flow-n">4</span>
       <span class="cn-flow-title">Publish to Jira</span>
       <span class="cn-flow-desc">Human paste of wiki markup; import path writes drafts only.</span>
       <span class="cn-flow-cmd">canonic convert</span>
     </li>
   </ol>

What you need → what you run
----------------------------

.. grid:: 1 2 2 2
   :gutter: 2

   .. grid-item-card:: Validate before merge
      :class-card: sd-border-0

      Front matter, ``resp-`` id, and team closings.

      .. raw:: html

         <ul class="cn-cmd-list"><li>canonic check<span class="cn-cmd-hint">exit 1 on findings</span></li></ul>

   .. grid-item-card:: Markdown → Jira wiki
      :class-card: sd-border-0

      Pandoc ``jira`` writer for a human to paste.

      .. raw:: html

         <ul class="cn-cmd-list"><li>canonic convert PATH</li></ul>

   .. grid-item-card:: Find similar answers
      :class-card: sd-border-0

      BM25 search and near-duplicate pairs.

      .. raw:: html

         <ul class="cn-cmd-list"><li>canonic search "…"</li><li>canonic dedupe</li></ul>

   .. grid-item-card:: Pull existing Jira text
      :class-card: sd-border-0

      REST read path → drafts under ``corpus/imports/``.

      .. raw:: html

         <ul class="cn-cmd-list"><li>canonic import-jira "JQL"</li></ul>

Install
=======

.. code:: shell

   cargo install --git https://github.com/HaoZeke/canonic
   # or from a checkout:
   cargo build --release

**Requirements:** Rust 1.75+, `pandoc <https://pandoc.org/>`_ for ``convert``,
optional `Vale <https://vale.sh/>`_ for style lint (Harper is linked in-process).
Run ``canonic doctor`` to see what the environment can already do.

First canned response
=====================

.. raw:: html

   <ol class="cn-steps">
     <li><strong>Create</strong> <code>corpus/responses/resp-example-topic.md</code> with front matter
     (<code>id</code>, <code>title</code>, <code>prefix: resp</code>, <code>tags</code>, <code>sop</code>) and a
     team-generic closing (e.g. <code>Support Team</code>).</li>
     <li><strong>Validate:</strong> <code>canonic check</code></li>
     <li><strong>Index &amp; search:</strong> <code>canonic reindex</code> then <code>canonic search "example topic"</code></li>
     <li><strong>Convert</strong> (needs pandoc): <code>canonic convert corpus/responses/resp-example-topic.md</code></li>
   </ol>

Delete the example once you commit a real response under its own id.
Full command reference: :doc:`usage`.

Documentation map
=================

.. grid:: 1 2 2 2
   :gutter: 2

   .. grid-item-card:: Usage
      :link: usage
      :link-type: doc

      CLI overview, corpus layout, dedupe, Jira import, exit codes.

   .. grid-item-card:: Design
      :link: design
      :link-type: doc

      Why Tantivy, markdown-as-source, and the review-before-migrate rule.

.. toctree::
   :maxdepth: 1
   :caption: Guides
   :hidden:

   usage
   design

Source & license
================

- Repository: https://github.com/HaoZeke/canonic
- License: MIT — see ``LICENSE``
- Site theme: `Shibuya <https://shibuya.lepture.com/>`_ (build with ``./docs/build.sh``)
