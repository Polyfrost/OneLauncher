## 2.9.0 (2026-10-25)

### Features

- Added themes by [ghost](https://github.com/ghost)
- Dedicated-folder clusters now keep their own resource packs and shaders instead of sharing the global ones by [Wyvest](https://github.com/Wyvest)
- Fabric Loader now auto updates on OneClient clusters and asks before updating custom and modpack clusters by [LynithDev](https://github.com/LynithDev)
- Added a Flatpak bundle to every release by [LynithDev](https://github.com/LynithDev)
- Added minecraft crash handling in launcher
- Added an ability to export your OneClient clusters, compatible with other launchers too
- Added syncing up dedicated clusters with shared directory data (settings, worlds, config etc.) by [rozwader](https://github.com/rozwader)
- Added thanos mod for troubleshotting cluster mods (binary search)
- Added the Wardrobe, where you can equip Poly+ cosmetics, change your Minecraft cape, save outfits and skins, and buy cosmetics by [Deftu](https://github.com/Deftu)
- The wardrobe preview shows Poly+ cosmetics and plays emotes on a new 3D player renderer by [LynithDev](https://github.com/LynithDev)

- Clusters can import modpacks

- Added importing instances from other launchers, such as: Modrinth App and Prism Launcher

### Fixes

- Added organizations showing up in mods pages in browser by [rozwader](https://github.com/rozwader)
- Fixed the browser replaying its page animation and losing the picked cluster when switching cluster or type by [rozwader](https://github.com/rozwader)
- Fixed launching with half-updated bundle mods: bundle content is now fully downloaded before any of it is swapped in, so a failed or offline download keeps the current mods and still launches, and a failure while swapping is retried once and stops the launch if it still fails by [Wyvest](https://github.com/Wyvest)
- Added warning when a user is trying to install a different version of already bundled mod
- Added a "Deleted" tab in mods list, where deleted bundled mods are stored by [rozwader](https://github.com/rozwader)
- Game logs also hide the session ID Minecraft prints on startup by [lunaynx](https://github.com/lunaynx)
- Added changelog button for mods in the update modal by [rozwader](https://github.com/rozwader)
- Added dependency checking when turning off a mod
- Disabled ability to search CurseForge on Orinthe clusters (CurseForge doesn't provide Orinthe compatible mods) by [rozwader](https://github.com/rozwader)
- Fixed mods auto enabling (after being disabled) when added from declined bundles (eg. skyblock bundle) by [rozwader](https://github.com/rozwader)
- Added option to choose from dedicated and shared dir while creating OneClient cluster instance by [rozwader](https://github.com/rozwader)
- Fixed too long names overflowing in the drag and drop window by [rozwader](https://github.com/rozwader)
- Added `ONECLIENT_DISABLE_JAVA_AUTOINSTALL` and `ONECLIENT_LAUNCHER_EXE` environment overrides for packagers by [saadndm](https://github.com/saadndm)
- Added ability to export clusters list of mods to clipboard
- Fixed filters resetting when you change a tab in cluster view by [rozwader](https://github.com/rozwader)
- Added hidden packages filter turning on when user searches packages thru searchbar, by [rozwader](https://github.com/rozwader)
- Fixed a black bar at the top and offset content on Windows with Intel integrated graphics by [LynithDev](https://github.com/LynithDev)
- Fixed resource packs and shaders edited in the file explorer being overwritten with the original on launch by [Wyvest](https://github.com/Wyvest)
- Fixed opening folders and links in external programs on Linux by [LynithDev](https://github.com/LynithDev)
- Added ctrl + f shortcut for focusing the searchbar in logs and live log views by [rozwader](https://github.com/rozwader)
- Fixed sort by not applying to OneClient versions in the instance list by [rozwader](https://github.com/rozwader)
- Added open link button for new OneClient version toasts by [rozwader](https://github.com/rozwader)
- Added a button for opening logs from launcher by [rozwader](https://github.com/rozwader)
- Added a dropdown for opengl/vulkan graphic backend in cluster with version 26.2 and above by [rozwader](https://github.com/rozwader)
- Added a Select button to the content list for enabling, disabling or deleting several items at once by [Wyvest](https://github.com/Wyvest)
- Changed the order of settings in cluster settings by [rozwader](https://github.com/rozwader)
- Fixed text wrapping up weirdly in the new instance creator by [rozwader](https://github.com/rozwader)
- Added the native shadow, border and rounded corners to the window on Windows by [LynithDev](https://github.com/LynithDev)

- Added a cluster reset button, that resets cluster back to it's factory settings: mods, config, packages.

- Keep Microsoft accounts signed in across token renewals by [chivenant](https://github.com/chivenant)

## 2.8.0 (2026-10-09)

### Features

- Added safe multiple minecraft instances launching, only dedicated clusters tho by [rozwader](https://github.com/rozwader)

### Fixes

- Added better informations about errors from mclogs. by [rozwader](https://github.com/rozwader)
- Added status bar showing up if a player is logged out or he has a stale refresh token by [rozwader](https://github.com/rozwader)
- Added a filter for choosing modpack loaders by [rozwader](https://github.com/rozwader)
- SkyBlock content installed without asking is removed from 26.3 clusters once, and turning an optional bundle down removes its mods
- Fixed shield.io badges not rendering properly in markdown viewers by [rozwader](https://github.com/rozwader)

## 2.7.0 (2026-10-05)

### Features

- Code blocks are selectable, and the open link confirmation has a copy button by [LynithDev](https://github.com/LynithDev)
- Game settings have a Game Arguments field for passing extra arguments to Minecraft, globally or per cluster by [Wyvest](https://github.com/Wyvest)
- Game logs hide account tokens, and the game output starts with the launch command ready to paste into a terminal by [LynithDev](https://github.com/LynithDev)
- GitHub hosted mods have a "View in browser" option that opens their repository by [LynithDev](https://github.com/LynithDev)
- The changelog page now shows each GitHub release's notes by [LynithDev](https://github.com/LynithDev)
- The mods folder now syncs with the launcher: jars added, renamed or replaced by hand show up in the Mods list, and jars deleted by hand are switched off instead of being put back by [Wyvest](https://github.com/Wyvest)

- Choose which GPU the game renders on by [LynithDev](https://github.com/LynithDev)

### Fixes

- Icons show on Linux systems with a comma decimal locale by [LynithDev](https://github.com/LynithDev)
- The "Uploaded to mclo.gs" notification no longer repeats when switching back to the Logs tab by [LynithDev](https://github.com/LynithDev)
- Open folder and open in browser work again on Linux by [LynithDev](https://github.com/LynithDev)
- Optional bundles are now offered before the first launch of every new version instead of being installed or skipped silently by [Wyvest](https://github.com/Wyvest)
- System information in the settings sidebar shows a tooltip explaining it can be clicked to copy by [LynithDev](https://github.com/LynithDev)
