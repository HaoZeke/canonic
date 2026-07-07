---
id: resp-small-compute-sbu-calculation
title: Small compute request needs a basic SBU calculation
prefix: resp
tags: [allocation, small-compute, sbu, demo]
sop: none
---

# Small compute request needs a basic SBU calculation

Hello,

Small compute applications may request up to **1,000,000 total SBUs** across partitions. GPU nodes consume the budget faster than CPU nodes. As a reference scale from current onboarding guidance:

- About **512 SBUs** for one hour on a four-A100 GPU node class used in training examples.
- About **128 SBUs** for one hour on a 128-core thin/Rome-class CPU node used in the same examples.

We need a short, plausible calculation before we can progress the review:

1. Number of runs or jobs.
2. Expected wall-time per run.
3. Resource type (CPU or GPU partition class) and scale (nodes or GPUs).
4. Total SBU estimate and a small margin for failed runs if relevant.

Small grants are also the usual path to gather scaling and utilization evidence before a large allocation. If you do not yet have performance data, please structure this request as a measurement campaign rather than a full production budget.

Please reply with the four items above (a short table or bullet list is fine).

Regards,
Support Team
