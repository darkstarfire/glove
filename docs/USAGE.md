# Using Glove — new features (phases 10 to 15)

*Em português: [USO.md](USO.md)*

This guide covers what arrived from phase 10 on (v0.10.1 to now): how to open each feature,
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
- **Click the desktop to show it:** in Preferences ▸ Desktop & Screen Saver, turn the option on
  and a click on an empty area of the desktop moves the windows aside (like F11); another click
  brings them back.
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
- Move the pointer to the top edge: the menu bar slides down and the Dock slides up, and the window
  **adjusts** to the space between them (nothing is covered, neither the app's top nor its bottom).
  Moving the pointer away, they leave and the window takes the whole screen again. The window
  changes size once each way, not every frame, so heavy apps (browser, video) don't stutter.
- The same shortcut, or Window ▸ Exit Full Screen, returns to the previous size.
- Apps that ask for full screen themselves (Firefox's F11, videos) work too.

## Jangada (the browser)

The **Jangada** icon in the Dock opens the Glove OS browser (package `jangada`, its own repository:
github.com/fabioaacarneiro/jangada): the Chromium engine (today's web features) in a Glove window,
with tabs, the address bar and Jangada's own pages, in the classic or the Modern look.

- **No title bar**, like Safari: close, minimize and zoom sit in Jangada's own toolbar, next to
  back, forward and home (the little house). Dragging the toolbar (or the free space of the tab
  strip) moves the window; a double click zooms it.
- **Address bar**, like Safari's: out of focus it shows only the site, with the padlock; click it
  (or Ctrl+L) and type an address or a search (DuckDuckGo). Esc goes back. Reload and stop are
  inside it.
- **Start page** of its own: the logo, the search and site shortcuts. When a site does not open,
  the **error page** is Jangada's, with Try Again.
- **Tabs** like Safari's: all the same width, the current one highlighted, the others a shade
  lighter than the strip, the site icon and the title in the middle, the × on hover and the + right
  after the last one. Ctrl+T opens, Ctrl+W closes, Ctrl+Shift+T reopens the last closed one,
  Ctrl+Tab goes to the next; links that open a new window become tabs. The **middle button** (or
  Ctrl + click) opens a link in a background tab; on a tab, it closes it.
- **Favorites:** the star in the address field (or Ctrl+D); the favorites bar (Ctrl+Shift+B), the
  Favorites menu and the start page show them.
- **History:** the menu has the latest sites, **Reopen Last Closed Tab**, **Reopen Tabs from Last
  Session** (the ones open when Jangada closed) and **Show All History** (Ctrl+H), a window with
  search, open in new tab and remove.
- **Find on page** (Ctrl+F), **Print** (Ctrl+P, through Glove's Print sheet) and **Save as PDF**
  (File menu, to the Downloads folder).
- **Google account:** Jangada ▸ Sign In to Google… (or Preferences ▸ Accounts) opens Google's
  sign-in; once signed in, Gmail, YouTube, Drive and the other services open with your account.
  Accounts also lets you manage the account and sign out. (Chrome's bookmark and password sync is
  not available in Jangada's engine.)
- **Preferences** (Ctrl+,): home page, reopen last time's tabs, page size, the search engine
  (DuckDuckGo, Google, Bing, Ecosia, Brave Search, Startpage), block windows pages open by
  themselves, Do Not Track, clear history and cookies, and Jangada's extensions: the **ad and
  tracker blocker** (on) and **dark mode on pages**. (Chrome Web Store extensions do not work yet
  in the windowless engine.)
- **Shortcuts:** Ctrl+R or F5 reloads, Alt+← and Alt+→ go back and forward, Alt+Home goes home,
  Ctrl+= / Ctrl+- / Ctrl+0 change the text size.
- **Right click:** Glove's menu (floating, above everything) with open link in new tab, copy link,
  copy, paste, search the selected text, back and reload.
- **Cursor:** the hand over links and the text bar over fields (for all of Glove).
- **Downloads** go straight to the Downloads folder; the **Downloads** window (Ctrl+J, or the
  arrow at the right of the toolbar) shows the progress and lets you open, show in folder and cancel.
- **Page notices** (alert and confirm) show as Glove alerts, in their own window.
- **About Jangada**, in the menu with its name, shows the version and the engine.
- **Light:** no GPU process, few page processes and nothing running in the background; only the
  parts of the page that changed go to the screen, and when idle the window is not redrawn. To use the
  graphics card: `JANGADA_GPU=1 jangada`.
- Opening it again (from the Dock or `jangada address`) opens a tab in the window already open. On
  Glove OS it is the system's default browser.
- Without the package, the icon opens Firefox or another installed browser. The choice in
  Preferences (`navegador = "..."`) comes first.

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

## Document editor (phase 12)

The Text Editor became a document editor: books, college papers, forms and reports without
another program. New documents save as **RTF** (opens in Word, LibreOffice and TextEdit);
`.txt` stays plain text.

- **Format bar** (above the ruler): paragraph style, font (opens the Fonts panel), size,
  B/I/U, text color, highlight, alignment, line spacing and list.
- **Characters**: Format > Font (Bold Ctrl+B, Italic Ctrl+I, Underline Ctrl+U,
  Strikethrough, Bigger/Smaller, Baseline, Show Colors Ctrl+Shift+C, Highlight).
- **Paragraphs**: alignment (Ctrl+Shift+L/E/J/R); on the ruler, drag the bar (first line)
  and the triangles (indents), click to add tab stops (drag off to remove); Format > Text >
  Spacing… for line spacing, before/after and indents in cm.
- **Styles**: Body, Title, Chapter, Section, Subsection, Quote (bar or Format > Style).
- **Lists**: bullets or numbers; Tab at the start of an item indents, Shift+Tab outdents,
  Enter on an empty item ends the list.
- **Pages**: View > Page Mode (Ctrl+Shift+W) and Zoom; File > Page Setup (Ctrl+Shift+P);
  Insert > Page Break (Ctrl+Enter).
- **Header, footer, page number and table of contents** are page elements you add or remove from
  the **Insert** menu (checked when present) and set up with a **double-click** on them:
  - **Header…** and **Footer…**: text on the left, center and right ({página}, {páginas}, {título}
    and {data} fields), the font, size, bold and italic, color, distance from the page edge, side
    inset, a rule separating it from the text and a different first page (cover). **Remove** takes
    it out.
  - **Page Number…**: top or bottom, left, center or right, the format ("1", "Page 1", "Page 1 of
    3"…), the first page's number and whether it shows on it.
  - **Table of Contents…**: the title, how deep it goes (chapters, sections, subsections) and whether
    it shows page numbers; with one in the document, the same window updates or removes it.
- **Notes**: Insert > Footnote… (Ctrl+Alt+N; double-click the mark to edit).
- **Tables**: Insert > Table…; Tab moves between cells (adds a row at the end); drag column
  borders; Format > Table for rows, columns, merge/split, borders and equal widths.
- **Zoom**: in the bar at the bottom of the window, − and + and the percentage menu (in page
  mode also Fit Width); Ctrl + mouse wheel zooms around the pointer; Ctrl+Shift+. zooms in,
  Ctrl+Shift+, zooms out and Ctrl+0 goes back to 100%. When the page is wider than the window
  a bottom scroll bar appears. The bar also shows the page and the word count.
- **Images**: Insert > Image… (PNG, JPEG or WebP) or drag the image into the document. It
  sits on the text line (center the paragraph to center it); click it to select and drag the
  corner to resize. It goes along in the RTF, the PDF and printing.
- **Table of contents**: Insert > Table of Contents… and Update Table of Contents.
- **ABNT** (Brazilian academic standard): File > New with ABNT Template, or Format > Apply
  ABNT Formatting.
- **Print and export**: Print (Ctrl+P) uses the laid-out pages; File > Export as PDF (vector,
  embedded fonts). The Editor's format is RTF.

## Screenshots with drawing

- **Timer for every mode:** Screenshot ▸ When ("Now", "In 5 seconds", "In 10 seconds") also works
  for "An area" and "A window": time to open a menu or set the screen up.
- **The screen freezes:** when choosing the area (or window) starts, Glove takes a picture of the
  screen at that instant and shows it still under the film; the capture comes from that picture, even
  if something changes afterwards (a menu closing, a video playing).

- The **Screenshot** app opens with **An area** selected (then A window and Full
  screen): press Enter or Capture and drag the area.
- Screenshots are saved in **Pictures › Screenshots** (the folder is created the first time); a
  screenshot edited in the Viewer is saved there too.
- Every screenshot (from the app or the shortcuts) goes to the clipboard right away:
  just paste it with Ctrl+V into a chat, e-mail or editor.
- The screenshot opens in the **Viewer** with the pen ready: pick the **color** and the
  **stroke width** in the strip at the top and draw on the image. Undo (Ctrl+Z), Clear
  and **Save** (Ctrl+S), which overwrites the screenshot and copies the image with the
  drawings. Ctrl+C copies it as shown, even without saving.
- **Shapes** (in the toolbar, next to Draw): freehand, **line**, **dashed line**, **arrow**,
  **rectangle** and **ellipse**, from where you press to where you let go. With **Shift**, the
  rectangle becomes a square, the ellipse a circle and lines snap straight (horizontal, vertical or
  45°).
- **Zoom** (also while drawing): the − and + toolbar buttons, or **Ctrl + mouse wheel**, which zooms
  in and out around the pointer without distorting the picture. Bigger than the window, scroll bars
  appear; the wheel scrolls (Shift for sideways) and dragging outside draw mode moves the picture.
- On any image open in the Viewer: the **Draw** button (Ctrl+D). JPEG photos, or ones
  reduced on opening, are saved to a new file next to them, "name (desenho).png".

## Pop-up lists

- The choice lists in Glove's apps (Appearance, Keyboard layout, Font…) open above everything and can
  go past the window edges, like on the Mac: the chosen item sits over the button. A list taller
  than the screen shrinks to fit between the menu bar and the Dock and scrolls (wheel, or rest the
  pointer on the top or bottom arrow). Arrow keys and Enter choose, Esc or a click outside closes.
- The lists and menus of other apps (GTK, Qt, browsers) can also go past their window, get the
  keyboard while open and close on a click outside, in another app or on the desktop.

## Open and Save

- Outside apps (VS Code, the browser and others that use the system portal) show Glove's
  Open/Save window. It can be resized by its edges or by the bottom-right corner; the wider
  it is, the more folder columns it shows.
- In Glove apps, Open/Save is a sheet that drops from the title bar. Drag the sheet's
  bottom-right corner to change its size (up to the size of the app window).

## Where new windows appear

- Every new window opens centered in the usable area (between the menu bar and the Dock) of
  the main display, whether the app is Wayland or X11.
- If the app changes the window size right after opening (common with X11 apps), the window
  goes back to the center with the new size. Once you move it, Glove leaves it alone.
- When the app already has a window at the center, the next one cascades: one step down and
  to the right for each window of the same app still in place. A window of another app always
  opens centered, and so do dialogs.
- An app that remembers it was zoomed opens zoomed, filling the usable area.

## When a display is unplugged

- Windows that were on the display that went away move to the one that stayed, centered on it.
- A window larger than the remaining display shrinks to fit: at most 90% of the usable area
  (between the menu bar and the Dock).
- A zoomed (maximized) window stays zoomed, filling the usable area of the remaining display;
  leaving zoom brings it back to a size that fits there.
- A full-screen window stays full screen on the remaining display.
- Windows on the display that stayed keep their place on it (even if it becomes the main
  one). This also applies to minimized and hidden windows and to those on other desktops.

## Resizing windows

- On a window edge the pointer turns into the double-headed arrow (↔, ↕ or diagonal)
  and stays that way while you drag. Near the corners the edge resizes diagonally, in
  both directions at once.
- The Preferences window can be enlarged too, from the edges or the bottom-right corner.
  Each pane's own size is the minimum: the window never gets smaller than that. What you
  enlarged is kept when you switch panes; if you only widened it, the height keeps following
  each pane. Wider, "Show All" fits more icons per row and the toolbar shows more favorites;
  taller, long panes show without scrolling. Each pane's lists and tab boxes grow along,
  filling the whole window.

## Control Center

The two-switch icon in the menu bar (next to the clock) opens the **Control Center**, like on newer
Macs:

- **Wi-Fi** (with the network name), **Bluetooth** and **Do Not Disturb**: click to turn on or off.
- **Display** (brightness), **Keyboard light** (only on computers that have one) and **Sound**: drag
  the track.
- It opens right away with what Glove already knows; the Wi-Fi, Bluetooth and sound state is checked
  in the background and shows a moment later. If the system's Bluetooth service (bluetoothd) is not
  running, the Bluetooth button stays off.

**Keyboard light:** **Ctrl + brightness keys** raise and lower the keyboard light (on Chromebooks and
laptops with a backlit keyboard), with the on-screen notice; dedicated keyboard light keys work too.

## Files: full names, several items and "Show in folder"

- Files opens in **list** view (change it in View, or with the toolbar buttons).
- **Full name:** resting on a cut name (in the list, columns, icons and every app's Open and Save
  sheets) shows a balloon with the whole name.
- **Several items:** **Ctrl + click** adds or removes an item; **Shift + click** picks the range.
- Other apps' **"Show in folder"** (the folder icon in Firefox's downloads, VS Code's…) opens Files
  at the folder with the file chosen.

## AppImage

An **AppImage** (a program in one file) opens with a **double click** in Files or on the desktop: a
window shows the program's icon and the **Applications** folder (`~/Aplicativos`). **Drag the icon
onto the folder** (or click Install) to copy it there; it then shows in All Apps. **Open Without
Installing** just runs it. Older AppImages need libfuse2, which Glove OS ships (without it, Glove
still runs them by unpacking the file first).

## Firefox with the Glove look

Glove OS no longer ships Firefox (the browser is Glove's own), but whoever installs it from the App
Center gets the **Glove theme**: brushed metal behind the tabs and toolbar, a
white address field with the blue focus ring and all three window buttons, like the classic Mac
browser. To go back to the default theme, use Settings › Extensions & Themes › Themes in Firefox.

## Disks and startup disks

**Disks** (in the Dock, in Utilities) lists the disks on the left (internal disks, USB sticks, cards
and disk images in use) and shows the chosen disk on the right: model, size, table (GPT or MBR),
connection and the **volume map**, with partitions sized to scale and the **free space**. Click a
volume to see its contents (Ext4, FAT, NTFS…), device, partition type, UUID, how much of the disk it
takes (%) and where it is mounted. **How full each volume is** shows as a strip under it on the map
and, in the details, as a bar ("X used (Y%), Z free"). Mounted volumes are measured by themselves;
for unmounted ones, **Measure Usage** mounts read-only, quietly (no icon on the desktop), measures
and unmounts. Mount, Unmount, Open, Eject and **Check** (checks the file system and repairs errors)
are under the map and in the Volume menu.

**Disk health:** under the disk's name, "Health: Good", "Good, with warnings" (bad sectors) or
"Failing" (copy your files now), with the temperature. **Details…** (or Disk ▸ Disk Health…) shows
the power-on time, bad sectors, NVMe SSD wear and the last test, and starts the disk's own **Short
Test** (about 2 minutes; the disk stays usable).

**Disk images** (Disk menu): **Create Disk Image…** copies the whole disk into an `.img` file (an
exact copy, for safekeeping); **Restore Disk Image…** writes an image back, block by block (asks
first; erases the disk). Both ask for the administrator password.

**Partitions and formatting** (choose a piece of the map; the buttons below change with it):

- **Format Disk…** (at the top): a new **GPT** table (UEFI computers, recommended), **MBR** (old
  computers, TVs, stereos) or none; option to overwrite with zeros. Erases the whole disk.
- **Create Partition…** (with a **Free** space chosen): size in GB (the suggestion is all the
  space), format (**Ext4** for Linux and Glove OS, **FAT32** for everything with files up to 4 GB,
  **exFAT** for USB sticks and cards, **NTFS** for Windows, **Btrfs**) and name.
- **Format…**: erases the volume and recreates it in the chosen format, with a name.
- **Resize…**: grows (up to the end of the free space right after it) or shrinks the volume along
  with its file system; the files stay. To shrink, the volume is unmounted and checked first. This
  is how to make room for Glove OS next to Windows: shrink the Windows (NTFS) partition and the
  installer uses the free space. Back up anything important first.
- **Edit…**: the partition type (Linux, Windows, FAT32, EFI System, swap, BIOS boot) and the volume
  name.
- **Delete**: deletes the partition (asks first); the space becomes free.

System volumes in use (`/`, `/boot`, `/home`…) cannot be formatted, deleted or resized. Changing
internal disks asks for the administrator password; USB sticks do not.

**Create a startup disk** (a USB stick that starts the computer, in UEFI), like Rufus:

1. Choose the USB stick on the left and click **Startup Disk…** (or Disk ▸ Create Startup Disk…).
2. Choose the image (`.iso` or `.img`; the sheet opens in Downloads).
3. Read the warning: it says which disk will be **erased** (name, device and size). **Erase and
   Write** starts; the footer shows the progress and has **Cancel**.

Disks recognises the image by itself:

- **Linux** (Glove OS, Debian, Ubuntu, Fedora… "hybrid" ISOs): written whole, block by block, and
  checked at the end (read back and compared). Starts in UEFI and BIOS. Asks for the administrator
  password (writing the whole disk needs it).
- **Windows installer** (10 and 11): the stick becomes GPT with a FAT32 partition and the ISO's
  files. If `install.wim` is over 4 GB (FAT32's limit), it is split into parts (`install.swm`…) by
  `wimtools`, which the Windows installer accepts. Starts UEFI computers, **even with Secure Boot
  on**, without WoeUSB. Computers with only an old BIOS will not start this stick.

The system disk in use (with `/`, `/boot`, `/home`…) is never offered for erasing. For scripts:
`glove-disks --gravar image.iso /dev/sdX` does the same in the terminal.

## Glove OS updates

Glove moves at its own pace, without waiting for Debian: for every new version, GitHub publishes
`glove` and `glove-os` in a **Glove OS apt repository** (signed), which Glove OS already has set up
(`/etc/apt/sources.list.d/glove-os.sources`). Debian 13 keeps coming from Debian.

- **Notice:** every now and then (2 minutes after login and every 6 hours) Glove checks for a new
  version. If there is one, a notice ("Glove OS 0.12.50 available") shows in the corner and in the
  menu bar's counter. **View** opens the pane. Each version notifies once.
- **System updates:** Glove OS lets the system refresh its package list every day (Debian's
  `apt-daily`, which installs nothing), and Glove checks that list every hour. When something new
  shows up, a notice says "3 system updates available", with the names.
- The notice stays in the **notification center** (the balloon in the menu bar) after the bubble goes
  away: clicking it there also opens Software Update.
- **Preferences ▸ Software Update:** at the top, **Glove OS**: the installed version, the new one,
  **what's new** and the **Update Glove** button (updates only Glove, from its repository). Below,
  the whole system's updates and **Install All**. **Check Now** asks the repositories again (`apt
  update`). All without the Terminal: the administrator password is asked in a sheet of the window
  itself, and installing shows a **progress bar** with what is being downloaded and installed. At
  the end, **Log Out…** to come back on the new Glove.
- **Glove on another Debian or Ubuntu:** the pane offers **Add the Glove Repository**, and from then
  on Glove updates through apt too (without the Glove OS identity, which only the `glove-os` package
  brings). The Terminal commands are at https://fabioaacarneiro.github.io/glove.
- From the Terminal the usual also works: `sudo apt update && sudo apt upgrade`.
- **Glove on Fedora and Arch:** the pane also offers **Add the Glove Repository** (once) and then
  **Update Glove**. With the repository, Glove comes along with the system updates (`sudo dnf
  upgrade`, `sudo pacman -Syu`). Updates are checked against the Glove key, which ships in the
  package.
- **Glove on Slackware:** there is no repository; **Update Glove** downloads the new version's
  package from the releases page, checks its checksum (`SHA256SUMS`) and installs it with
  `upgradepkg`.
- **Glove installed from source** (`./glove-setup-tool.sh install`): the pane tells you about the new
  version and how to update (`git pull` and install again).
- Outside apt, updating shows a spinner instead of the progress bar.

## System password (administrator requests)

When a program needs administrator permission (formatting a disk in Disks, installing with
Flatpak, changing a system network, `pkexec` in the Terminal), the **Authentication** window opens
with what was asked. Type your password and click **Authenticate**; **Cancel** (or Esc) denies the
request. The password is checked by the system itself (polkit and PAM); Glove does not keep it.
Accounts that are not administrators see an administrator's name and need that password.

Glove's own apps that need an administrator (Software Update, Startup Items ▸ Services, Security,
App Center, Installer) use the same **Authentication** window, separate from the app, instead of a
sheet inside their window. A wrong password is reported right there. The other passwords open in it
too: a Wi-Fi network's, changing the account password (Preferences ▸ Accounts), changing the
Keychain password, Startup Disk and Software Update's **Install All** on Fedora, Arch and openSUSE
(these two used to open the Terminal to ask for the password).

## Password after sleep and hibernation

With **Preferences ▸ Security ▸ Require password to wake from sleep or screen saver**
turned on:

- The screen locks before any system sleep or hibernation, whoever asks for it: the Sleep
  menu item, closing the lid, the power button, an empty battery or `systemctl hibernate`.
  On wake you get the password panel, never the desktop.
- **Sleep** (main menu): the lock screen shows, the display turns off 3 seconds later and then the
  computer sleeps. If it can't sleep, the display stays off and the session stays locked.
- Closing the lid with an external monitor connected only turns off the laptop screen (you
  can keep working on the external one). If the external monitor goes away while the lid is
  closed, the screen turns off and locks right away.

## Wallpaper: several displays, panorama and animated

In Preferences ▸ Desktop & Screen Saver ▸ Desktop:

- **Favorites** (the first collection): the pictures you gathered, in a grid. The last cell, **+
  Add…**, opens the Open window to pick a picture (PNG, JPEG or WebP) from any folder; it joins the
  Favorites and becomes the wallpaper. In the other collections, right-click a picture to add it to
  the Favorites. With the pointer over a favorite, the **×** in its corner removes it from the
  collection (the file stays where it is).
- **Displays** (shown with more than one display, in extend mode):
  - **The same picture on all:** each display shows the wallpaper at its own size;
  - **A picture for each display:** click a display in the miniature, then a picture;
  - **One picture across all (panorama):** one picture spans the displays, in their real
    positions. Two 1920×1080 displays side by side make 3840×1080: a picture that size shows
    whole, half on each. One above the other: 1920×2160.
- **Fit:** Fill Screen (crops the excess), Fit to Screen (whole, with bars), Stretch, Center and
  Tile.
- **Videos and Animations:** videos (MP4, WebM, MKV, MOV…) and GIFs from your Videos and Pictures
  folders become an animated wallpaper (needs ffmpeg). Options: frames per second, **Light** (half
  resolution, good for simple computers), **Pause on battery** and **Pause with a full-screen app**.
  The first time, Glove prepares in the background a copy of the video already at screen size (in
  `~/.cache/glove/papel`, no visible loss); with it the wallpaper uses about 5 times less CPU.
  It also pauses with the screen locked and with "Reduce effects". Animated works in all three
  modes (in panorama, a wide video spans the displays).
- The Pictures folder accepts PNG, JPEG and WebP.

## Simpler computers (phase 13)

- Apps no longer redraw the window just because the mouse moved over it: if nothing
  changes on screen, they spend nothing. On a weak CPU that was the heaviest cost.
- Apps redraw only the part of the window that changed (the typed letter, the selected
  row), not the whole window. If an app ever shows a stale piece on screen, run it with
  `GLOVE_FULL_REDRAW=1` (the old way) and report it; `GLOVE_CHECK_DAMAGE=1` checks every
  frame and prints to the terminal when it finds a difference.
- Dragging windows no longer stutters once a second (the compositor's safety net was
  dropping the frame in flight; fix by darkstarfire).
- **Preferences > Appearance > Reduce effects**: no animations (minimize, Exposé,
  desktops, All Apps), no Dock magnification, no bouncing icon on launch, no window
  shadows and no pulsing alert button. Takes effect right away, no restart.
- **Preferences > Startup Items** (also under Accounts, or Activity Monitor > Process > Login
  Items… / Services…): the **Applications** tab lists what opens by itself when you log
  in and how much memory each one uses now; the **Services** tab lists system services
  that tend to weigh (printing, Bluetooth, AnyDesk, Docker, SSH, automatic updates…), with
  state and memory. Unchecking turns it off now and on later boots; system services ask
  for the administrator password.
- To find what still weighs: run an app with `GLOVE_PROF=1` in the terminal (prints the
  time of each frame) or the whole session with `GLOVE_PROF=1`.

## Installing Glove from the ISO

1. **Download the ISO** for the version from the [Releases](https://github.com/fabioaacarneiro/glove/releases) page (GitHub builds one for
   every version; check it with `sha256sum -c SHA256SUMS`) **or build it** (on the computer with the
   source code): `./glove-setup-tool.sh iso`. It goes to `iso/glove-os-VERSION-amd64.iso`.
2. **Try it in a virtual machine** before using it on a real computer: `./glove-setup-tool.sh iso test`
   (needs QEMU: `sudo apt install qemu-system-x86 ovmf`). The 32 GB virtual disk is
   `iso/disco-teste.qcow2`. GNOME Boxes and VirtualBox also open the ISO.
3. **Write it to a USB stick** (erases the stick): `sudo dd if=iso/glove-os-VERSION-amd64.iso
   of=/dev/sdX bs=4M status=progress oflag=sync`, replacing `sdX` with the stick (check with `lsblk`).
4. **Start the computer from the stick** (the computer's boot menu). The Glove live system logs in
   by itself and opens **Install Glove OS**. You can use the live system before installing; the
   installer is also in Quick Search. On the live system the user is `glove` and the password is
   **`live`** (for `sudo`, console logins, and unlocking if you lock the screen with Super+L);
   `root` has no password. On the live system, the screen blanking by itself and sleep do not ask
   for a password when coming back.

The ISO looks like Glove OS from start to finish, with nothing to set up: the boot menu with the
wallpaper, the system's boot and shutdown screen (Plymouth, with the Glove theme) and the login
screen. The installed system is the same: GRUB with the wallpaper and the Glove boot screen.

The installer asks for:

- **Language and keyboard** of the installed system (with a field to test the keyboard).
- **Time zone:** search for the city.
- **Disk:**
  - **Erase the whole disk and install Glove**: the red warning shows which disk will be erased;
  - **Install alongside the current system**: uses the disk's largest free space. Needs UEFI, an EFI
    partition and 20 GB free. To make that space, click **Prepare the disk with Disks…**: in Disks,
    choose the Windows (NTFS) partition, **Resize…** it smaller and close Disks; the installer's list
    updates by itself and offers the free space. (Windows can also shrink it, in "Disk Management ▸
    Shrink Volume".)
  - **Install on a chosen partition**: formats only the chosen partition (20 GB or more) and installs
    there; the disk's other partitions stay as they are. Needs UEFI and an EFI partition on the disk
    (Windows', for example). Good for reusing an old Linux partition, or one made first in Disks.
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
| Alt+Tab | Strip with the current app's windows (like Super+Tab, with thumbnails). Holding Alt, Tab and Shift+Tab (or ← →) choose, Q closes the chosen window, Esc cancels; releasing Alt brings the chosen one |

## Modern icons (Mide)

In **Preferences › Appearance › Icons**, besides Glove's own theme (the default, period look), there
are the **Mide** icons: **Mide**, **Mide Claro** (light) and **Mide Escuro** (dark), more modern,
drawn by the Mide project (based on WhiteSur, with no Apple artwork). They come with Glove OS and
arrive with updates (the `glove-mide-icons` package). Glove's apps use Mide's own drawings when it
has one (Files, Terminal, Text Editor, Preferences…). The Preferences panes switch to the chosen
theme's icons too, and in the Dock each icon's shadow follows the new drawing.

## Modern appearance (Light and Dark)

In **Preferences › Appearance › Appearance**, besides the period themes (Light, Pinstripes, Metal and
Graphite; Light is still the default), there are **Modern Light** and **Modern Dark**: flat windows
with rounded corners, flat close, minimize and zoom buttons, a translucent menu bar, rounded menus, a
frosted-glass Dock, flat buttons and checkboxes and thin scroll bars. Dark turns windows graphite with
light text. It goes well with the Mide icons (Mide Claro and Mide Escuro). The pop-up menus (such as
Appearance, Highlight color and Icons) are flat too, with the arrows in a small square in the
highlight color. The menu bar logo is flat: black in Modern Light, white in Modern Dark (in the
period themes, the gel glove).

**Use the appearance everywhere** (right below the appearance choice): session startup, the lock
screen and the log-out screen follow the appearance (in Modern: flat panels, a flat light or
graphite background and the flat glove). The login screen and system startup and shutdown
(Plymouth) change too: they ask for the administrator password and can take a minute (the system
rebuilds its boot image). Turned off, these screens go back to the classic look.

