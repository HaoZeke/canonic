Design notes
============

- **Tantivy BM25** for search and near-duplicate discovery (better fit for
  curation/dedupe than a hand-rolled store).
- Markdown remains the source of truth; Jira is a publication surface.
  ``canonic convert`` produces wiki markup for a human to paste in;
  ``canonic import-jira`` reads existing issue comments back out as drafts.
  Neither direction writes to Jira automatically.
- Quality checks implement the meeting rule: **review before migration**, shared
  ``resp`` prefix only.

Citation
--------

See ``CITATION.cff``, or use GitHub's "Cite this repository" button.

License
-------

MIT — see ``LICENSE`` in the repository root.
