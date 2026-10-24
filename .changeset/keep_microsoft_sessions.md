---
default: patch
---

# Keep Microsoft accounts signed in across token renewals

Account expiry now follows the Minecraft token (24h) instead of the Microsoft token (1h), and a rejected refresh token is kept and retried instead of deleted, so one failed renewal no longer forces a fresh sign-in.
