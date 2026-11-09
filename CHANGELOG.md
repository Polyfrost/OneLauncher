## 2.10.0 (2026-11-08)

### Features

- Added ability to add custom shared directories for clusters
- Added translations into the client by [rozwader](https://github.com/rozwader)
- Added a Help menu to the macOS menu bar by [tunaseckin](https://github.com/tunaseckin)
- Share instances with a code and sync them across your computers by [Deftu](https://github.com/Deftu)
- Added social and party features to the launcher by [rozwader](https://github.com/rozwader)
- The content list can be sorted by date added or by recently updated by [tunaseckin](https://github.com/tunaseckin)

- Themes match OneConfig and can be customised by [LynithDev](https://github.com/LynithDev)

### Fixes

- Fixed the launcher going black on Windows with AMD iGPUs by [LynithDev](https://github.com/LynithDev)
- Cached browser results no longer blink when switching pages by [junapur](https://github.com/junapur)
- The browser now has tabs for mods, textures, shaders, datapacks, and modpacks instead of a dropdown by [junapur](https://github.com/junapur)
- A bundle catalog with an archive that failed to download no longer gets published halfway by [Wyvest](https://github.com/Wyvest)
- A bundle's package now takes over from an older copy you installed yourself by [LynithDev](https://github.com/LynithDev)
- Adding a mod no longer reports a dependency as missing when a bundle already ships it, like Fabric Language Kotlin on 1.8.9 by [tunaseckin](https://github.com/tunaseckin)
- Censored the signed-in username in the game log by [LynithDev](https://github.com/LynithDev)
- Fixed cosmetic previews in the wardrobe being off-center by [junapur](https://github.com/junapur)
- Every setup step now has a colorful illustration that matches the welcome screen by [LynithDev](https://github.com/LynithDev)
- The content list toolbar drops the Select button, and category tabs no longer have a scrollbar drawn over them by [LynithDev](https://github.com/LynithDev)
- Open the game output log by default on the logs page by [LynithDev](https://github.com/LynithDev)
- Added "Disable anyway" when trying to disable a dependency, it only disables the dependency without disabling all of its mods by [rozwader](https://github.com/rozwader)
- Added an option in launcher settings to not show live log when lauching a cluster by [rozwader](https://github.com/rozwader)
- The duplicate mods sheet marks which copy is Loaded, dims the Skipped ones, and shows the date and time each was added
- Package galleries show small previews right away and no longer download the same image twice, and screenshot thumbnails stay loaded when coming back to the page by [LynithDev](https://github.com/LynithDev)
- Fix the Flatpak publish workflow failing to upload the bundle to the release by [LynithDev](https://github.com/LynithDev)
- Update Freya to 0.5.0 by [LynithDev](https://github.com/LynithDev)
- Launch the game with a UTF-8 locale on Linux so non-ASCII text and paths work regardless of the system locale by [LynithDev](https://github.com/LynithDev)
- Fixed the opengl/vulkan dropdown overriding the graphic backend picked in game, it now reads and writes options.txt directly by [LynithDev](https://github.com/LynithDev)
- The Minecraft theme's pixel icons are redrawn by hand so they line up and stay symmetrical by [LynithDev](https://github.com/LynithDev)
- Fixed horizontal scrollbar in logs view being hard to click by [rozwader](https://github.com/rozwader)
- Show native JVM crash logs (hs_err_pid) under crash reports by [LynithDev](https://github.com/LynithDev)
- When users tries to download new java runtime, the jdk's he already has are flagged as "Installed" by [rozwader](https://github.com/rozwader)
- Updates no longer bring back a desktop shortcut you deleted on Windows by [tunaseckin](https://github.com/tunaseckin)
- Added a language picker to onboarding and the settings, listing the community translations by [rozwader](https://github.com/rozwader)
- The launcher stays responsive while it checks game files before a launch by [tunaseckin](https://github.com/tunaseckin)
- Fixed folders opening in a terminal and Minecraft failing to open folders from the AppImage on Linux by [LynithDev](https://github.com/LynithDev)
- The Linux tray icon no longer needs GTK or libappindicator by [LynithDev](https://github.com/LynithDev)
- Fixed loader update modal button order by [rozwader](https://github.com/rozwader)
- Fixed long log names overlapping the dropdown arrow by [junapur](https://github.com/junapur)
- Added manual update button in cluster settings
- Added option to truncate the log it the middle if user wants to export it to mclogs, as well as keeping just the beginning by [rozwader](https://github.com/rozwader)
- A mod that is installed both from a bundle and as your own copy now shows as one entry, with a "copies" button to manage each copy separately by [LynithDev](https://github.com/LynithDev)
- The Minecraft themes use pixel-art icons, and custom themes can turn them on in the theme editor by [LynithDev](https://github.com/LynithDev)
- Made small text larger in the Minecraft themes by [LynithDev](https://github.com/LynithDev)
- The Minecraft theme centres its text properly, spaces icons in buttons evenly, and uses a pixel-art logo with the name in the bold Minecraft font by [LynithDev](https://github.com/LynithDev)
- Added mojang as a jdk provider by [rozwader](https://github.com/rozwader)
- Added new cluster picker instead of dropdowns by [rozwader](https://github.com/rozwader)
- Removed radio buttons from category rows in new instance creator
- Setup now has an Appearance step where you can pick a theme, and theme cards show their names by [LynithDev](https://github.com/LynithDev)
- The setup screens show the OneClient logo icon next to the name again by [LynithDev](https://github.com/LynithDev)
- Cleared the window to opaque black instead of transparent by [LynithDev](https://github.com/LynithDev)
- Added Ctrl+Shift+O (Cmd+Shift+O on macOS) to open the logs folder by [LynithDev](https://github.com/LynithDev)
- Package names and icons show from the local cache right away instead of waiting on Modrinth and CurseForge by [LynithDev](https://github.com/LynithDev)
- The Minecraft theme's Appearance icon is now a recognisable paintbrush by [LynithDev](https://github.com/LynithDev)
- Minecraft-theme pixel icons stay crisp at fractional display scaling like 125% by [LynithDev](https://github.com/LynithDev)
- Made polyplus 'turn off' warning show only once, if a player still disables it
- Prerelease versions now sort below their release when checking dependency ranges by [Wyvest](https://github.com/Wyvest)
- Added server and client side filters for mods in browser by [rozwader](https://github.com/rozwader)
- Reset only the settings that fail to parse instead of the whole file by [LynithDev](https://github.com/LynithDev)
- Minecraft taking too long to close is now explained instead of shown as an unknown crash by [tunaseckin](https://github.com/tunaseckin)
- Lists no longer re-render on every frame of a page or tab transition by [LynithDev](https://github.com/LynithDev)
- Scrolling with a mouse wheel is now smooth everywhere in the launcher by [LynithDev](https://github.com/LynithDev)
- Social and party views now follow the active theme's corners, text size and icons by [rozwader](https://github.com/rozwader)
- Social sidebar now uses the same popup chrome and backdrop as the control center by [rozwader](https://github.com/rozwader)
- Saving a custom theme no longer pauses the launcher while the file is written by [LynithDev](https://github.com/LynithDev)
- Every light/dark theme pair now gets its own System option, theme cards have a menu to duplicate, export, and delete custom themes, and the control center has a dark mode toggle by [LynithDev](https://github.com/LynithDev)
- Instance sharing and cloud sync screens can be translated by [Deftu](https://github.com/Deftu)
- Fixed mods published on CurseForge being flagged as unpublished when importing a modpack by [rozwader](https://github.com/rozwader)
- Added version and loader info when exporting the clusters list of mods by [rozwader](https://github.com/rozwader)
- Added version + loader info on home page for OneClient clusters by [rozwader](https://github.com/rozwader)
- Fixed wardrobe card border hidden behind the bottom button by [junapur](https://github.com/junapur)
- Fixed the wardrobe crashing when switching between sections by [junapur](https://github.com/junapur)
- Show a loading spinner on the wardrobe page and fade it in once loaded by [LynithDev](https://github.com/LynithDev)
- Right-clicking a wardrobe item opens its menu by [LynithDev](https://github.com/LynithDev)
- Fix changing your skin from the wardrobe, which Minecraft had started rejecting with a sign-in error by [LynithDev](https://github.com/LynithDev)
- Added warning when user is about to import a modpack/cluster that shows if there are any mods not published by Modrinth/CurseForge by [rozwader](https://github.com/rozwader)
- The first setup screen now greets you with a simple animated welcome, and the rest of the screen fades in after it by [LynithDev](https://github.com/LynithDev)

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
- The wardrobe lets you pick the particle color of your aura, and the 3D preview shows it by [LynithDev](https://github.com/LynithDev)

- Clusters can import modpacks

- Added importing instances from other launchers, such as: Modrinth App and Prism Launcher

### Fixes

- Added organizations showing up in mods pages in browser by [rozwader](https://github.com/rozwader)
- Fixed the browser replaying its page animation and losing the picked cluster when switching cluster or type by [rozwader](https://github.com/rozwader)
- Bundle updates download the largest files first so they finish sooner by [tunaseckin](https://github.com/tunaseckin)
- Fixed launching with half-updated bundle mods: bundle content is now fully downloaded before any of it is swapped in, so a failed or offline download keeps the current mods and still launches, and a failure while swapping is retried once and stops the launch if it still fails by [Wyvest](https://github.com/Wyvest)
- Added warning when a user is trying to install a different version of already bundled mod
- Added a "Deleted" tab in mods list, where deleted bundled mods are stored by [rozwader](https://github.com/rozwader)
- Game logs also hide the session ID Minecraft prints on startup by [lunaynx](https://github.com/lunaynx)
- Added changelog button for mods in the update modal by [rozwader](https://github.com/rozwader)
- Added dependency checking when turning off a mod
- The create dialog shows the Java version a game version actually launches with by [tunaseckin](https://github.com/tunaseckin)
- Disabled ability to search CurseForge on Orinthe clusters (CurseForge doesn't provide Orinthe compatible mods) by [rozwader](https://github.com/rozwader)
- Fixed mods auto enabling (after being disabled) when added from declined bundles (eg. skyblock bundle) by [rozwader](https://github.com/rozwader)
- Switching an instance to a dedicated folder offers to bring its shared worlds and settings along by [tunaseckin](https://github.com/tunaseckin)
- Added option to choose from dedicated and shared dir while creating OneClient cluster instance by [rozwader](https://github.com/rozwader)
- Fixed a test that depended on JSON key order by [tunaseckin](https://github.com/tunaseckin)
- Fixed too long names overflowing in the drag and drop window by [rozwader](https://github.com/rozwader)
- Added `ONECLIENT_DISABLE_JAVA_AUTOINSTALL` and `ONECLIENT_LAUNCHER_EXE` environment overrides for packagers by [saadndm](https://github.com/saadndm)
- Added ability to export clusters list of mods to clipboard
- Fixed filters resetting when you change a tab in cluster view by [rozwader](https://github.com/rozwader)
- Stopping a game from the launcher lets it save and close before forcing it to quit on macOS and Linux by [tunaseckin](https://github.com/tunaseckin)
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
- Fixed the color picker and dropdowns opening outside the window on high-DPI displays by [LynithDev](https://github.com/LynithDev)
- Changed the order of settings in cluster settings by [rozwader](https://github.com/rozwader)
- Desktop shortcuts for modpack clusters now use the modpack's icon by [LynithDev](https://github.com/LynithDev)
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
