---
id: resp-gpfs-performance-first-checks
title: GPFS or project-space slowness first checks
prefix: resp
tags: [gpfs, performance, project-space, demo]
sop: https://confluence.example.com/display/DEMO/Filesystem+performance
---

# GPFS or project-space slowness first checks

Hello,

When a ticket reports slow project space or GPFS behavior, separate **system
symptoms** from **application changes** before escalating.

1. Ask whether the same workflow was faster on a prior code or data revision.
2. Check the published monitoring views (for example Grafana) and site health
   checks for the period of the slowdown.
3. Confirm the job used the intended filesystem path and was not thrashing a
   shared scratch or home area by mistake.
4. If the problem stays undefined or looks systemic, follow the filesystem
   performance SOP (`sop` in front matter) rather than spending unbounded time
   on one-off experiments.

Please send: job IDs or time window, project space path, and whether anything
changed in the application or input size. We will take it from there.

Regards,
Support Team
