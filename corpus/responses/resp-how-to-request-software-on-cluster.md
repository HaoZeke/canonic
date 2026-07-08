---
id: resp-how-to-request-software-on-demo
title: How to request a software install on the cluster
prefix: resp
tags: [software, install, modules, demo]
sop: none
---

# How to request a software install on the cluster

Hello,

For a software install or module on the cluster, open a service-desk ticket with:

- Package name and version (or git commit / tarball URL).
- License terms if the software is not fully open source.
- Build preferences (toolchain, MPI, CUDA, or site modules you already use).
- A short note on who will maintain the install for the project.

We will check whether an existing module or EasyBuild recipe covers the need
before building something custom. If a Confluence SOP for software installs is
maintained for your team, link it in the ticket; this response uses `sop: none`
until that page is attached in the shared library.

Regards,
Support Team
