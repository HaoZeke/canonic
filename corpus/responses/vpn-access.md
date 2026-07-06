---
id: vpn-access
title: Corporate VPN onboarding
tags: [network, vpn, remote]
---

# Corporate VPN onboarding

Hello,

For remote network access install the corporate VPN client:

1. Download the package from the software catalog (search **WireGuard profile corporate**)
2. Import the configuration named `corp-primary.conf`
3. Authenticate with your SSO credentials and the hardware token

Split tunnel is enabled by default. Full tunnel is required only for finance systems on `10.40.0.0/16`.

If the tunnel connects but DNS fails, flush the local resolver cache and retry. Open a ticket with the word **vpn_dns_failure** if the issue persists after reboot.

Regards,
Network Operations
