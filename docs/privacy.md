## Privacy

RemixClient is offline-first. **There is no telemetry and no analytics by default.**

### Stored on your computer only

- Minecraft accounts: username, UUID and — for Microsoft accounts — the OAuth tokens,
  in the launcher's credentials store under your user profile.
- Launcher settings, instances, worlds, logs, crash reports and world backups.
- Locally picked skins and capes for offline accounts.

Nothing in that list leaves your machine unless a feature below says otherwise.

### When the launcher talks to the internet

| When | What is sent | Can you avoid it? |
|---|---|---|
| You sign in with Microsoft | The normal OAuth and Xbox Live flow | Yes — use an offline account |
| Browsing mods / bundles / metadata | Your request, plus the usual IP and user agent | Use the launcher offline |
| Checking for updates | Current version | Disable update checks |
| Discord Rich Presence | Game/instance name, only while Discord desktop is running | Turn it off in settings |
| Crash reports | Only if you enable a Sentry DSN or press "report" yourself | Off by default |

Offline accounts do **not** contact Microsoft or Mojang at all. Their skin is served
from a loopback HTTP server on your own machine (`127.0.0.1`), only while the launcher
is running, and only to the game process.

### Logs

Launcher logs and crash reports are written locally. Usernames are censored in
instance logs before they are shown or shared. Review a file before you send it to
anyone.

### Contact

Open an issue in the project repository if you have a privacy question.
