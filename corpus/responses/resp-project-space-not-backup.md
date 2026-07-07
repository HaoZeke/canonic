---
id: resp-project-space-not-backup
title: Project space is not a backup or archive
prefix: resp
tags: [storage, project-space, demo]
sop: none
---

# Project space is not a backup or archive

Hello,

Project space on the cluster is **user-managed working storage**. It is more persistent than shared scratch, but it is **not** a backup system and **not** a long-term research-data archive.

Please note:

- Home directories (default quota on the order of **200 GB**) are backed up on a regular schedule.
- Shared scratch (on the order of **8 TiB**) is volatile; files may be removed after roughly **14 days** without access.
- Project space is appropriate for collaborative working data. You own backup and retention.
- For long-term retention, use an approved archive service such as the tape archive (small-application context discussed up to tens of TB, subject to current policy).

If data is deleted by a workflow, written to the wrong path, or lost outside the storage system's redundancy guarantees for disk failure, Support does not promise recovery from project space alone.

Please confirm how you will back up any data you cannot afford to lose, then resubmit or reply with that plan if this ticket is about a storage or recovery request.

Regards,
Support Team
