# Using Glove — new features (phase 10)

*Em português: [USO.md](USO.md)*

This guide covers what arrived in phase 10 (v0.10.1 to v0.10.9): how to open each feature,
what it does and its shortcuts. Shortcuts with **Super** use the Windows key; with Glove running
in a window inside another desktop, **Alt** works as Super. Every shortcut can be changed in
**Preferences ▸ Keyboard ▸ Keyboard shortcuts**.

## All Apps (app grid)

Every installed program, full screen, in pages, over the blurred wallpaper. The Dock stays in
front (clicking an app in it opens the app and closes the grid).

- **Open:** the rocket icon in the Dock (right after Files), the **F4** key or a hot corner
  (Preferences ▸ Exposé & Dashboard ▸ Hot corners ▸ All Apps). On the touchpad, **pinch with four
  fingers**.
- **Search:** just type; the search shows every app, including the ones in folders.
- **Choose:** arrows and Enter, or click.
- **Pages:** mouse wheel, two fingers on the touchpad, Page Up/Down or the dots at the bottom.
- **Folders:** clicking a folder opens a strip below it with its apps. Click the name (in the
  strip's corner) to rename it. Click outside to close the folder. System apps start in the
  Utilities folder.
- **Arrange:** **hold** an icon until they all wiggle (or just start dragging):
  - drag to move it; at the left or right edge, the page turns;
  - pause **over another app** and let go: both become a folder, already open so you can name it;
  - drop on a folder to put the app in it;
  - drag an app out of the strip to take it out of the folder (an empty folder goes away);
  - **Esc** or a click on empty space stops the wiggle.
  The arrangement is saved (`~/.config/glove/grade.json`); apps installed later go at the end
  (utilities, in the Utilities folder).
- **Close:** Esc (first clears the search, then closes the folder and stops the wiggle), F4 again
  or a click outside the icons.

## Desktops

Several desktops, each with its own windows. There are 4 by default; change the number in
Preferences ▸ Exposé & Dashboard ▸ Desktops (1 turns them off).

- **Switch desktops:** **Ctrl+Super+←** and **Ctrl+Super+→**. The windows slide sideways and a
  panel in the middle of the screen shows which desktop you are on. On the touchpad: **three
  fingers sideways**.
- **Take the window along:** add **Shift** (Ctrl+Super+Shift+← or →).
- **In Exposé (F9):** the strip at the top shows the desktops with their windows in miniature.
  - Click a desktop to go to it.
  - Drag a window onto a desktop to send it there (it shrinks over the strip).
- **Find a window:** clicking its app in the Dock, choosing it with Super+Tab or picking the
  window in the **Window** menu takes you to the desktop it is on. The Window menu lists the
  windows on other desktops with the desktop number.

A new window always opens on the current desktop. Minimized and hidden windows come back on the
current desktop.

## Quick Look (Files)

See a file without opening its program.

- In Files, select an item and press **Space** (or File ▸ Quick Look, Ctrl+Y).
- It shows pictures (with their dimensions), the first page of PDFs, the start of text and code
  (scroll with the wheel) and, for anything else, the large icon with kind, size and date.
- The **arrow keys** move to the next item, both in the Quick Look window and in Files.
- **Open** opens it in its app. **Space** or **Esc** close.

PDF previews use `pdftoppm` (the `poppler-utils` package).

## Notification center

Glove shows app notifications: Firefox, Thunderbird, `notify-send` and Glove's own apps.

- Each notification appears in a **bubble** at the top right for 6 seconds. Critical ones, like
  low battery, stay until you click.
- **Clicking the bubble** answers the app and brings the app forward. The **×** dismisses it.
- The **speech bubble in the menu bar** (right of the clock) shows how many are unread. Click it
  to open the center with the last 60.
- In the center, **Do Not Disturb** hides the bubbles, except critical ones. **Clear** empties
  the list. Esc or a click outside closes the center.

To try it: `notify-send "Hello" "A test notification"` in Terminal.

## Full screen

- **Ctrl+Super+F** or **Window ▸ Enter Full Screen** puts the front window on the whole monitor,
  without a title bar. The menu bar and the Dock go away.
- Move the pointer to the top edge to bring the menu bar (and the Dock) back.
- The same shortcut, or Window ▸ Exit Full Screen, returns to the previous size.
- Apps that ask for full screen themselves (Firefox's F11, videos) work too.

## Tabs (Files, Text Editor and Terminal)

The tab strip appears when there is more than one tab. Each tab has a **×** to close it, and the
**+** at the end opens another.

| | New tab | Switch tabs | Close the tab |
|---|---|---|---|
| Files | Ctrl+T (same folder), or "Open in New Tab" in a folder's menu | Ctrl+Page Up/Down | Ctrl+W |
| Text Editor | Ctrl+Shift+T (Open… also opens in a new tab) | Ctrl+Page Up/Down | Ctrl+W (asks if there are changes) |
| Terminal | Ctrl+Shift+T (each tab with its own shell) | Ctrl+Page Up/Down or Ctrl+Tab | Ctrl+Shift+W |

## Touchpad gestures

Only in the Glove session (not with Glove in a window inside another desktop).

| Gesture | What it does |
|---|---|
| 3 fingers up | Exposé (all windows); again to go back |
| 3 fingers down | The front app's windows |
| 3 fingers sideways | The neighbouring desktop (content follows your fingers) |
| Pinch with 4 fingers | All Apps |
| Spread 4 fingers | Show the desktop |

Turn them on and off in Preferences ▸ Trackpad ▸ Multiple fingers.

## Backups

Automatic backups of your home folder to an external disk, with a timeline.

1. Open **Backups** (All Apps ▸ Utilities, or Quick Search).
2. **Choose Disk…** and pick the external disk (or a folder).
3. **Back Up Now** makes the first backup. With **Back up automatically every hour** checked,
   Glove makes one every hour by itself while the disk is connected, and tells you in the
   notification center.

- Only what changed takes space: files identical to the previous backup become hard links.
- Every backup from the last 24 hours is kept, one per day for the last month and one per week
  before that. When the disk is nearly full, the oldest go away.
- **Timeline:** on the left, pick a backup. On the right, browse its folders (double-click goes
  in, **‹ Up** goes back).
- **Restore** puts the selected item back where it came from. If something with that name
  already exists, it comes back with "(restored)" in its name, so nothing is overwritten.
- **Show in Files** opens the backup in Files.

Backups live in `Glove Backups/<computer name>/<date>` on the disk. Caches, the Trash and program
folders (`.cache`, `node_modules`…) are skipped. The list is in `~/.config/glove/copias.toml`.
The disk must support hard links (ext4, btrfs, xfs…); on FAT/exFAT each backup takes its full
size.

## App Center

Install and remove programs with apt (system packages) and Flatpak (Flathub).

- **Search:** type a name and press Enter. Results from Flatpak and apt appear together.
- **Installed:** apt programs with windows and your Flatpaks. **Open** or **Remove** them here.
- **Updates:** **Check for Updates** fetches the new list (apt update) and **Update All**
  installs it.
- For apt, the App Center asks for the **administrator password** in a sheet. The password only
  goes to `sudo`; it is never shown or saved.
- Flatpak installs in your account, without a password. With no remote set up, use **Turn On
  Flathub**.

## Photos

Your Pictures folder as a library.

- Photos appear as thumbnails, grouped by month, newest first. The bar at the bottom changes the
  thumbnail size.
- In the sidebar, **Photos** shows everything, and each subfolder of Pictures becomes an
  **album**.
- **Double-click** (or Enter) opens the photo large:
  - the **arrow keys** move between photos;
  - **Space** turns the **Slideshow** on and off (4 seconds per photo);
  - **Esc** or **‹ Library** goes back;
  - **Open in Viewer** opens it in Viewer.
- Photos ▸ Look for New Photos (Ctrl+R) rereads the folder.

Formats: PNG, JPEG and WebP.

## Recorder

- The **red button** records; the same button (a square) stops. Recording ▸ Record (Ctrl+R) does
  the same.
- While recording: the time, the level LEDs and the waveform of the last seconds.
- Recordings go to **Music ▸ Recordings**, as WAV, with the date in the name.
- In the list: **Play/Stop** (or double-click, or Space), **Move to Trash** and **Show in
  Files**.

It records through PipeWire (`pw-record`); without it, it uses `parecord` or `arecord`.

## Messages

IRC chat.

1. The first time, the account sheet appears. The default is **Libera.Chat** (open and free),
   port 6697, with TLS. Pick a **nickname** and **channels** (for example `#debian`) and click
   **Connect**.
2. The sidebar holds the server (with its messages), channels and private conversations, with
   the unread count.
3. Type at the bottom and press Enter. Your messages appear on the right, in blue; everyone
   else's on the left.

- **+** opens a channel (`#name`) or a conversation with someone (their nickname).
- **Commands:** `/join #channel`, `/part`, `/msg nick text`, `/me action`, `/nick new`.
- A private message, or one with your nickname, while the window is in the background becomes a
  notification.
- **Account…** changes the server and nickname. The account is kept in
  `~/.config/glove/mensagens.toml`. NickServ passwords are not stored yet.

## Music

- **Library:** the Music folder and any you add (Music ▸ Preferences ▸ Add Folder…). Files without
  tags take the artist and album from the folders (Artist/Album/song).
- **Browser:** View ▸ Show Browser (Ctrl+B) puts Artists and Albums above the list.
- **Playlists:** the **+** at the bottom (or File ▸ New Playlist, Ctrl+N) creates one and asks for
  its name. Pick a song and use File ▸ Add to Playlist. Delete removes it from the playlist.
  Double-click the name to rename it. **Recently Played** keeps the last 50.
- **Shuffle and Repeat:** the buttons at the bottom (Repeat cycles through off, all and one) or the
  Controls menu.
- **Announcement:** with the window in the background, the new song shows in the notification
  center (it can be turned off in Preferences).
- Shortcuts: Space plays and pauses, Ctrl+→ and Ctrl+← change songs, Ctrl+↑ and Ctrl+↓ change the
  volume.

## Video

mpv shows the picture; the Video panel controls it. Formats: mp4, mkv, webm, avi, mov, m4v, ogv,
mpg, wmv, flv, ts and 3gp (mpv plays almost any codec).

- **Open:** File ▸ Open… (Ctrl+O), **Open Location…** (Ctrl+L, a link or path) and **Open Recent**.
- **Controls:** ← and → skip 10 s, Shift+← and Shift+→ skip 1 min, Home goes back to the start.
  Ctrl+← and Ctrl+→ open the previous and next video in the folder. **Speed** (0.5× to 2×, or [ and
  ]), **Loop**, volume (↑ and ↓) and **Mute** (M).
- **Video:** Full Screen (Ctrl+F), Size (Ctrl+0, 1 and 2), Aspect Ratio and **Capture Frame**
  (Ctrl+Shift+S, saved in Pictures).
- **Audio:** choose the track (dubbed, original, commentary) and adjust the delay.
- **Subtitles:** subtitles inside the file and the ones in the folder with a similar name load by
  themselves. In the menu: Show Subtitles (V), pick the track, **Load Subtitles…** (srt, ass,
  vtt…), delay and size.
- **Preferences** (Ctrl+,): resume where you left off, play the next video in the folder when one
  ends, use the graphics card and the preferred subtitle and audio languages (pt, en…).

## Installing Glove from the ISO

1. **Build the ISO** (on the computer with the source code): `./glove-setup-tool.sh iso`. It goes to
   `iso/glove-os-VERSION-amd64.iso`.
2. **Try it in a virtual machine** before using it on a real computer: `./glove-setup-tool.sh iso test`
   (needs QEMU: `sudo apt install qemu-system-x86 ovmf`). The 32 GB virtual disk is
   `iso/disco-teste.qcow2`. GNOME Boxes and VirtualBox also open the ISO.
3. **Write it to a USB stick** (erases the stick): `sudo dd if=iso/glove-os-VERSION-amd64.iso
   of=/dev/sdX bs=4M status=progress oflag=sync`, replacing `sdX` with the stick (check with `lsblk`).
4. **Start the computer from the stick** (the computer's boot menu). The Glove live system logs in
   by itself and opens **Install Glove**. You can use the live system before installing; the
   installer is also in Quick Search.

The installer asks for:

- **Language and keyboard** of the installed system (with a field to test the keyboard).
- **Time zone:** search for the city.
- **Disk:**
  - **Erase the whole disk and install Glove**: the red warning shows which disk will be erased;
  - **Install alongside the current system**: uses the disk's largest free space. Needs UEFI, an EFI
    partition and 20 GB free. The installer does not shrink partitions: free the space first, in
    the other system (on Windows, "Disk Management ▸ Shrink Volume") or with Disks.
- **Account:** name, user name, password and computer name. The account is an administrator (`sudo`).
- **Summary:** check it and tick "I understand. Install."

The installation shows its progress and the command log. At the end, **Restart** and remove the
stick. The installed system already has the chosen language, keyboard and time zone, without the
welcome assistant.

Notes:

- **Secure Boot:** the installed GRUB is not signed. Turn Secure Boot off on the computer.
- The installer only installs from the Glove live system. On any other system,
  `glove-instalador --simular` shows every page and the commands it would run, without changing
  anything.

## New shortcuts

| Shortcut | What it does |
|---|---|
| F4 | All Apps |
| Ctrl+Super+← / → | Previous / next desktop |
| Ctrl+Super+Shift+← / → | Move the window to the previous / next desktop |
| Ctrl+Super+F | Full screen |
| Space (Files) | Quick Look |
| Ctrl+T / Ctrl+Shift+T | New tab (Files / Text Editor and Terminal) |
| Ctrl+Page Up / Page Down | Previous / next tab |
