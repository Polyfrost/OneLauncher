---
default: patch
---

# Fixed launching with half-updated bundle mods: bundle content is now fully downloaded before any of it is swapped in, so a failed or offline download keeps the current mods and still launches, and a failure while swapping is retried once and stops the launch if it still fails
