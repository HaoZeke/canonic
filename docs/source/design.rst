Design notes
============

.. raw:: html

   <div class="cn-page-intro">
     <p>Three deliberate choices: markdown as the only source of truth, a local
     search index for curation, and never auto-writing Jira.</p>
   </div>

.. raw:: html

   <ul class="cn-principles">
     <li>
       <h3>Markdown is canonical</h3>
       <p>Jira is a publication surface. <code>convert</code> emits wiki markup for a human
       to paste; <code>import-jira</code> only writes review drafts under
       <code>corpus/imports/</code>.</p>
     </li>
     <li>
       <h3>Tantivy BM25 for curation</h3>
       <p>Search and near-duplicate discovery fit a local inverted index better
       than a hand-rolled store — with Jaccard as a second opinion on pairs.</p>
     </li>
     <li>
       <h3>Review before migrate</h3>
       <p>Quality checks encode the meeting rule: shared <code>resp</code> prefix only,
       valid front matter, team-generic closings — gate the library before Jira.</p>
     </li>
   </ul>

What canonic does **not** do
----------------------------

- It does **not** open, edit, or comment on Jira issues for you.
- It does **not** promote ``corpus/imports/`` drafts into ``corpus/responses/``.
- It does **not** replace human judgment on which answer is the team standard.

Citation
--------

See ``CITATION.cff``, or use GitHub's "Cite this repository" button.

License
-------

MIT — see ``LICENSE`` in the repository root.

Building this site
------------------

.. code:: shell

   ./docs/build.sh
   # open docs/build/index.html
