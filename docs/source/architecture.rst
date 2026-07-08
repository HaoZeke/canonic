Architecture
============

.. raw:: html

   <div class="cn-page-intro">
     <p>Markdown under a shared <code>resp-</code> prefix is the only source of truth.
     Quality gates and a local Tantivy index sit in front of Jira publication.</p>
   </div>

.. raw:: html

   <div class="cn-figure">
     <img src="_static/architecture.svg" width="840" height="280" alt="canonic architecture: markdown → quality → Tantivy → Jira" />
     <p class="cn-figure-caption">Pipeline: author in git → check/lint → search/dedupe → convert or import as review drafts. Neither direction writes Jira automatically.</p>
   </div>

Layers
------

+------------------+--------------------------------------------------+
| Layer            | Responsibility                                   |
+==================+==================================================+
| Corpus           | ``corpus/responses/resp-*.md`` + front matter   |
+------------------+--------------------------------------------------+
| Quality          | ``check`` (prefix/sop/closings), ``lint`` engines|
+------------------+--------------------------------------------------+
| Index            | Tantivy BM25 under ``.canonic-index/``           |
+------------------+--------------------------------------------------+
| Publish surface  | ``convert`` (pandoc jira) / human paste          |
+------------------+--------------------------------------------------+
| Import drafts    | ``import-jira`` → ``corpus/imports/`` only       |
+------------------+--------------------------------------------------+

Library map
-----------

The CLI is a thin clap front end over the ``canonic`` library crate:

.. raw:: html

   <div class="cn-figure">
     <img src="_static/modules.svg" width="720" height="220" alt="canonic library modules" />
   </div>

Full API documentation is generated with **rustdoc** and linked from
:doc:`api`.
