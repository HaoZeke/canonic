---
id: resp-gpfs-project-space-performance
title: Project-space or GPFS performance ticket triage
prefix: resp
tags: [storage, gpfs, performance, project-space, sop]
sop: none
---

# Project-space or GPFS performance ticket triage

Hello,

For slowdowns on project space or filesystem behaviour tickets, please help us separate **system symptoms** from **application changes**:

1. When did the slowdown start, and which path or project space is involved?
2. Did the application, input size, concurrency, or library stack change recently?
3. Can you compare against a prior application version or a smaller input that used to be fine?
4. Any job IDs, timestamps, and a short description of the I/O pattern (many small files, large sequential reads, metadata-heavy walks)?

Advisors will also check monitoring and test signals (for example ReFrame and Grafana views used operationally) and will fall back to the existing filesystem-performance SOP when the problem is undefined or looks systemic rather than application-specific.

Please reply with the four items above so we can route the ticket efficiently.

Regards,
Support Team
