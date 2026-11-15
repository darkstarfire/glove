# Glove

*Leia em português: [README.md](README.md)*

A lightweight Linux desktop written in Rust, with a look inspired by the "gel" era of the
2000s: translucent buttons, fine pinstripes, brushed metal, soft shadows and a magnifying
Dock. All artwork is original and drawn in code (Mac OS X Tiger is the style reference, never
copied); the only ready-made images are the glove logo and the Glove OS wallpaper.

The interface speaks **Portuguese and English** (it follows the system language) and new
languages are added with a `.po` file — see [Languages](#languages).

## Glove OS: a Brazilian distribution

**Glove OS** is a Linux distribution **made in Brazil**. Inside, it is Debian 13: the same
packages, repositories and updates, 100% compatible. Outside, it is Glove: a desktop written from
scratch in Rust (a Wayland compositor, an interface library and 38 apps of its own), with the
"gel" look of the 2000s and its own identity, from the boot menu to `fastfetch`. Portuguese is its
native language: texts are born in Portuguese in the code, and English comes as a translation.

It was created by a **Brazilian software engineer with more than 22 years on Linux** who was bored
and wanted to practice AI in "different places": instead of one more website or one more app, a
whole system, with a compositor, every pixel drawn in code, an installer and an ISO, built
together with an AI (Claude).

**Download:** each version's ISO is on the [Releases](https://github.com/fabioaacarneiro/glove/releases) page
(see [Glove OS (the distribution, as an ISO)](#glove-os-the-distribution-as-an-iso)).

## Current state

Look and behavior baseline: **Mac OS X 10.4 Tiger** (plus a Pinstripes theme for the
10.0–10.2 look). Phases 1 to 15 of the [plan](docs/PLANO.md) (in Portuguese) are done: the whole era (1 to 9.16), phase 10 (beyond
the era), the **Glove OS** installer and ISO (11), the document editor (12), simpler computers (13),
screenshots with drawing (14) and modern wallpaper (15). Steps 13.3, 13.4 and 14.3 are still open,
and the installer and ISO still need testing on real computers. How to use each feature:
[docs/USAGE.md](docs/USAGE.md).

**System (the `glove` compositor)**

- Its own Wayland compositor (Smithay), running as a **full session** (DRM/KMS + libinput) or
  **inside a window**
- **Multiple displays**: extend, mirror or main only; resolution and scale per screen; sharp on
  HiDPI and fractional scaling
- **X11 apps** through Xwayland, with Glove's title bar
- Global menu bar: main menu (Recent Items, Force Quit, Sleep, Shut Down…), the app menu, **each
  app's own menus**, submenus and keyboard navigation; Wi-Fi, volume, battery (time left, health and cycles, also for third-party batteries and Chromebooks whose firmware does not recognise the battery: the `glove-bateria` service reads it straight from the EC and shuts the computer down before it runs out) and display extras,
  clock and user name (fast user switching)
- Glass **Dock** with magnification, bouncing icons, context menus, auto-hide, bottom/left/right
  position, **Scale** and **Genie** minimize effects (deformation done on the GPU), live
  thumbnails and a Trash that fills up
- Windows with shadows, close/minimize/zoom, move (also across displays) and resize; **Super+Tab**
  app switcher; **Exposé** and hot corners
- **The desktop works like Files**: items from the Desktop folder and the disks, with menus, drag
  and drop, in-place rename, Arrange By, Clean Up and View Options (Ctrl+J)
- **Widget Dashboard** (F12): Clock, Calendar, Calculator, Weather (Open-Meteo), Note, System
  Monitor and Dictionary
- **All Apps** (F4 or the rocket in the Dock): every installed app full screen, in pages, over the
  blurred wallpaper, with search, folders (Utilities and your own) and drag-to-arrange (hold an
  icon until they all wiggle)
- **Wallpaper** the same on every display, one per display or as a **panorama** (one picture across
  the displays, side by side or stacked), and **animated** (a video, played from a light cached
  copy and paused when the screen is locked, on battery or with a full-screen app)
- **Desktops** (Ctrl+Super+arrows; with Shift the window comes along), also in a strip in Exposé,
  where windows can be dragged from one desktop to another
- **Notification center**: bubbles for app notifications (the standard Linux service, the one
  `notify-send` uses), the center in the menu bar and Do Not Disturb
- **Full screen** (Ctrl+Super+F) and three- and four-finger touchpad **gestures**
- **Quick Search** (Super+Space): installed apps, Preferences panes and files
- **Screenshots** of the whole screen, an area or a window, on any display
- Force Quit that detects unresponsive apps; screen lock (PAM), screen blanking, laptop lid,
  volume and brightness keys
- **System password** (`glove-senha`, the polkit agent): any program that asks for administrator
  permission through polkit (Disks, gnome-disks, Flatpak, NetworkManager, `pkexec`) shows Glove's
  password window
- Accessibility: zoom, inverted colors, large cursor
- Drag and drop between apps, text and file clipboard
- Glove's Open/Save sheet also for other apps (FileChooser portal: Firefox, Chromium…)
- **Light** (default, Tiger), **Pinstripes**, **Metal** and **Graphite** themes; icons from the **Glove**
  PNG icon theme (the default, generated from Glove's drawings and published as glove-icons; lighter
  than drawing them at run time) or another system theme
- **Reduce effects** (Preferences ▸ Appearance), for simpler computers
- Redraws only what changed: 0% CPU when idle

**Session and login**

- Its own login screen on greetd (`glove-login`), with session choice; a code-drawn Plymouth
  splash; first-run assistant; Shut Down/Restart with countdown

**Apps (all built with glove-ui and original artwork)**

| App | What it does |
|---|---|
| Files | The Tiger file browser: metal, sidebar, columns/list/icons, preview, copy/move with progress, Trash, Get Info, smart folders, compress, eject, color labels, Open With, Connect to Server (smb, sftp, ftp, webdav) and Network |
| Preferences | "Show All" grid with the era's panes (Appearance, Desktop & Screen Saver, Dock, Displays, Sound, Network, Power Saving, Keyboard, Mouse, Trackpad, Date & Time, Accounts, International, Startup Items, Accessibility, Terminal…) |
| Terminal | Alacritty core: truecolor, nvim/htop, mouse, OSC 52, themes, Nerd Font |
| Text Editor | RTF documents (open in Word and LibreOffice): fonts, paragraphs and ruler, styles, lists, tables, pictures, pages with header, footer and footnotes, ABNT template, PDF export; plain text too; Find/Replace |
| Viewer | Pictures (PNG, JPEG, WebP) and PDF, thumbnails, zoom, rotate; drawing on screenshots |
| Browser | Opens the browser that fits the machine (NetSurf with little RAM, Firefox above) |
| Mail | IMAP and SMTP with TLS, mailboxes, compose, reply and forward |
| Calendar and Contacts | Month grid with events (.ics) and cards (vCard) |
| Music | Library (the Music folder and any you add), Artists and Albums, playlists (M3U), shuffle and repeat; light pure-Rust player |
| Video | mpv shows the picture, Video controls it: menus, subtitles, audio tracks, speed, resume where you left off |
| Notes | Colored sticky notes, all in one process |
| Dictionary | Free StarDict and DICT dictionaries |
| Font Viewer and Characters | View, install and disable fonts; character map |
| Passwords | Passwords in the system keychain: view, copy, delete, lock and change the keychain password |
| Network Tools | Interface info, Ping, Lookup, Traceroute, Whois, Finger and port scan |
| System Monitor, Logs, System Information | Processes and graphs, journal logs, hardware |
| Disks | Through udisks2 (like gnome-disks): disks, images and the volume map (partitions and free space), mount, unmount and eject; formatting, partitions, resizing and writing ISOs come in phase 16 |
| Search, Screenshot, Archive Manager | Find files, capture the screen, compress/expand |
| Calculator, Clock, Chess, Help, About Glove | Scientific calculator, clock in the Dock, chess with 3 levels, searchable help |
| Backups | Hourly backups of the home folder to an external disk, timeline and Restore |
| App Center | Search, install, remove and update with apt and Flatpak (Flathub) |
| Photos | The Pictures folder as thumbnails by month, albums from subfolders and slideshow |
| Recorder | Records voice to WAV with level and waveform; play, delete |
| Messages | IRC chat (Libera.Chat by default, with TLS) in bubbles |
| Install Glove OS | From the USB stick (the ISO) to the disk: the whole disk or alongside another system |

Files, Text Editor and Terminal have **tabs**; Files has **Quick Look** (Space).

## Lightness

All 38 apps are one executable (`glove-apps`; `glove-files`, `glove-prefs`… are links to it, and
it opens the app by the name it was called with, like BusyBox). Why:

- **Disk:** every app uses the same interface and drawing library (buttons, icons, fonts, themes).
  With one executable per app, that code was repeated in each: 166 MB. Together it exists once:
  44 MB (about 55 MB with the compositor and the portal). Less to download in the ISO and updates.
- **Memory:** Linux loads an executable's code pages once and shares them among every process
  running it. Each open app no longer brings its own copy of the code; only its data (window,
  files, text) is its own. With Files, Preferences and the welcome window open, memory went from
  35 to 26 MB (−26%), and the saving grows with each app opened.
- **Build:** the library is linked once, not 38 times: from ~7 to ~2.5 minutes.
- **What doesn't change:** each app is still a separate process. If one hangs the others keep
  going, and Force Quit closes only that one.
- **The cost:** any change rebuilds the whole executable (31 MB), not just the changed app.

Font files are also mapped instead of copied (the same pages for every process). Idle, Glove uses
0% CPU: it only redraws what changes.

Each process's own memory (RssAnon), measured on 15/11/2026 in a real session (1920×1200 screen
at scale 1.5):

| Process | Memory |
|---|---|
| `glove` (compositor) | ~32 MB, including the video driver's heap (Mesa), textures and the wallpaper |
| `glove-prefs` | ~4 MB |
| `glove-calc` | ~2.6 MB |
| `glove-term` | ~1.5 MB idle; 200,000 lines in 0.16 s |

Details and how to measure (in Portuguese): [docs/MEMORIA.md](docs/MEMORIA.md).

## Running

The apps' library ([glove-ui](https://github.com/fabioaacarneiro/glove-ui)) is a git submodule
in `ui/`: clone with `git clone --recursive` (or run `git submodule update --init` afterwards). **After
every `git pull`**, update glove-ui too with `git submodule update --init` (`./glove-setup-tool.sh`
does it by itself); with an old glove-ui the build fails.

It needs **Rust 1.89 or newer** (what the dependencies require). Some distributions ship an older
Rust (Debian 13 has 1.85): install it with [rustup](https://rustup.rs) or run `rustup update`. New
Rust releases build Glove; each version's CI always uses the latest stable.

```sh
cargo build --release
./target/release/glove            # opens Glove in a window
./target/release/glove -c foot    # opening a program right away
```

## Installing

`glove-setup-tool.sh` builds the packages and also installs directly, on any Linux. It speaks
Portuguese when the system is in Portuguese and English otherwise. The package brings
everything: the programs, the "Glove" session on the login screen, the Glove login screen
(the `glove-login` service, on top of greetd, which comes as a dependency), the icon theme
and the documentation.

```sh
./glove-setup-tool.sh                 # menu with every option
./glove-setup-tool.sh deb             # pacotes/glove_VERSION_amd64.deb (Debian, Ubuntu, Zorin, Mint)
./glove-setup-tool.sh rpm             # Fedora/openSUSE (needs rpmbuild; without it, leaves the .spec)
./glove-setup-tool.sh arch            # Arch (.pkg.tar.zst and the PKGBUILD for makepkg)
./glove-setup-tool.sh slackware       # Slackware (.tgz)
./glove-setup-tool.sh all             # all four
./glove-setup-tool.sh install         # no package: builds and installs in /usr/local
./glove-setup-tool.sh uninstall       # the reverse of "install" (or calls the package manager)
./glove-setup-tool.sh deps            # what's missing to build (apt, dnf, pacman, zypper, xbps, apk)
./glove-setup-tool.sh grub            # Glove OS wallpaper on the GRUB screen ("grub remove" undoes it)
```

Installing the package (Debian/Ubuntu/Zorin example):

```sh
sudo apt install ./pacotes/glove_VERSION_amd64.deb
```

Then, on your usual login screen (GDM, LightDM, SDDM…), choose **Glove** from the gear menu.

**Glove login screen as the main one** (from the next boot):

```sh
sudo systemctl enable --force glove-login   # turn it on (replaces GDM/LightDM/SDDM)
sudo systemctl enable --force gdm           # go back to GDM (or lightdm, sddm…)
```

`./glove-setup-tool.sh login` does the same as the first command, but remembers the previous
login screen and brings your display, scale, keyboard and theme settings to it;
`./glove-setup-tool.sh login remove` goes back. The service uses its own greetd configuration
(it doesn't touch `/etc/greetd/config.toml`); to use your own, create `/etc/glove/greetd.toml`.

**Removing:** `sudo apt remove glove` (or `rpm -e`, `pacman -R`, `removepkg`). If the Glove
login screen was the main one, removing it switches back to the previous one, so the computer
is never left without a login screen. Personal settings (`~/.config/glove`) stay.

**GRUB screen (optional).** At the end, `./glove-setup-tool.sh install` asks whether to also
change the GRUB screen (the menu shown when the computer starts) to the Glove OS wallpaper, with
white menu text. `./glove-setup-tool.sh grub` does it later and `./glove-setup-tool.sh grub remove`
undoes it. It copies the image to `/boot/grub/glove-os.png`, points `GRUB_BACKGROUND` at it (in
`/etc/default/grub.d/glove.cfg`; on distributions without that folder, in marked lines at the end
of `/etc/default/grub`) and puts the text colors in `/etc/grub.d/06_glove`. GRUB only: with another
bootloader (systemd-boot…) the command says so and changes nothing. On the Glove OS ISO this is
already done: in the boot menu of the USB stick and in the installed system's GRUB.

**Boot screen (Plymouth).** The Glove theme (light grey background, the glove logo and a spinner) shows while
the system starts and shuts down. On another distribution, install it with
`sudo ./data/install-plymouth.sh` (`--remove` goes back to the previous theme). On the Glove OS ISO
it is already done, on the USB stick and in the installed system: Plymouth is in the package list,
the theme ships in the `glove-os` package (which turns it on with
`/usr/share/glove-os/tela-de-inicio`) and the installed system's GRUB passes `splash` to the kernel.

## Glove OS (the distribution, as an ISO)

**Glove OS** is the distribution: Debian 13 inside (same packages, repositories and updates, 100%
compatible) with Glove as the desktop and its own identity: the name "Glove OS" in `os-release`
(with `ID=debian`, for programs), the glove logo, fastfetch and neofetch with the
glove, the console and the GRUB menu. That identity comes in the `glove-os` package, which only
ships in the ISO; the `glove` package installed on another distribution does not rename it.

The ISO works on a USB stick or DVD, with Glove, Firefox and the installer. Booting from the stick
logs into Glove by itself and opens **Install Glove OS**, which copies the system to the disk
(whole disk or alongside another system). How to use the installer:
[docs/USAGE.md](docs/USAGE.md#installing-glove-from-the-iso).

```sh
./glove-setup-tool.sh iso        # builds the ISO into iso/ (asks for the sudo password; 15 to 40 min)
./glove-setup-tool.sh iso test   # starts the ISO in a virtual machine (QEMU) with a test disk
```

To build the ISO: a Debian-family system (Debian, Ubuntu…) on x86_64, with `dpkg-deb`, `sudo`,
`debootstrap` installed (`sudo apt install debootstrap`; live-build requires the system's one) and
Rust 1.89 or newer. The script downloads Debian's official live-build and debootstrap into
`iso/ferramentas` (other distributions' live-build is usually too old). Everything heavy stays in
`iso/`, outside Git.

On the live system the user is `glove` and the password is **`live`** (for `sudo` and console
logins); `root` has no password.

**Ready-made ISO:** for every new version (tag `v…`), GitHub Actions builds the ISO on Debian 13 and
publishes it, with the `.deb` packages and `SHA256SUMS`, on the [Releases](https://github.com/fabioaacarneiro/glove/releases) page
([.github/workflows/iso.yml](.github/workflows/iso.yml); the build takes about 15 minutes).

## Development session

```sh
cargo build --release
./install.sh                         # step by step: session, login screen, Plymouth
./target/release/glove --monitores    # shows the display, resolution and scale the session will use
```

`./install.sh` points the session at the project's own programs, so rebuilding updates it.
Session log: `~/.local/state/glove/sessao.log`.

## Shortcuts (defaults, editable in Preferences)

Inside another desktop, Alt works as Super.

| Shortcut | Action |
|---|---|
| Super+Enter | New terminal |
| Super+Q | Close window |
| Super+M | Minimize |
| Super+H / Super+Alt+H | Hide the app / hide the others |
| Super+↑ | Zoom |
| Super+Tab | Switch apps |
| Alt+Tab | Switch between the front app's windows |
| Ctrl+Super+F | Full screen |
| F12 | Widget Dashboard |
| F4 | All Apps |
| Ctrl+Super+← / → | Previous / next desktop (with Shift, takes the window) |
| F9 / F10 / F11 | Exposé: all windows / app windows / show the desktop |
| Super+, | Preferences |
| Print / Shift+Print | Screenshot of the screen / of an area |
| Ctrl+Alt+Esc | Force Quit |
| Ctrl+Alt+Backspace | Quit Glove |
| Super+Space | Quick Search |
| Super+L | Lock the screen |
| Super+Alt+Space | Next keyboard layout |
| Super+Alt+8 / = / − | Screen zoom on, in, out (Ctrl+Super+Alt+8 inverts colors) |
| Ctrl+F2 | Open the menu bar from the keyboard |

## Languages

Glove follows the system language (`LANG`, which the International pane sets); `GLOVE_LANG`
forces one. The Portuguese text stays in the code and is the key (`tr!("Abrir")`); translations
live in `locales/<language>.po` (gettext format). A language without a translation falls back
to English. Dates, number separators and app names follow the language.

To add a language: copy `locales/en.po` to `locales/<code>.po` (for example `es.po`), translate
the `msgstr` lines and put it in `~/.local/share/glove/locales/` to try it without rebuilding;
to ship it, leave the file in `locales/`: every `.po` there is built into the program (the
`build.rs` of `glove-i18n`, in `ui/crates/i18n`). `cargo test -p glove-i18n`
fails if any marked text has no English translation, and `tools/i18n.py` helps mark new texts.

## Configuration

`~/.config/glove/config.toml` (created on first run). Preferences edits this file and Glove
reloads it when it changes. Everything in the file can also be set in Preferences. Examples:

```toml
[energy]
screen_off_minutes = 10   # blank the screen after N idle minutes (0 = never)
require_password = true   # ask for the password when coming back (blank screen, sleep, hibernation, lid closed)

[hot_corners]             # Exposé in the corners: "", "todas" (all), "app" or "mesa" (desktop)
bottom_right = "mesa"
```

Unlocking checks the password through PAM. `data/install-session.sh` installs the
`/etc/pam.d/glove` service; without it, Glove uses the `login` service.

## Layout

```
ui/                glove-ui submodule: glove-ui (toolkit), glove-paint (drawing, icons, themes),
                   glove-config (shared settings) and glove-i18n (translations)
crates/compositor  the compositor (`glove`)
crates/apps        `glove-apps`: one executable for every app
crates/portal      `glove-portal` (FileChooser portal) and `glove-escolher`
crates/login       login screen (`glove-login`, on greetd; `--fake` to try it)
crates/arte        `glove-arte`: Plymouth theme, the "Glove" icon theme and its preview sheet
crates/energia     battery (library used by the compositor and Preferences)
crates/instalador  Install Glove OS (`glove-instalador`)
crates/senha       system password: the polkit agent (`glove-senha`)
crates/…           one folder per app (files, prefs, term, editor, viewer, correio, agenda…)
data/              session, greetd, Plymouth, GRUB, wallpapers, the Glove OS identity and the ISO
ci/iso.sh          builds the ISO in continuous integration (.github/workflows/iso.yml)
tools/i18n.py      helper to mark texts for translation
```

## Login screen in development mode

With Glove installed, the login screen is the `glove-login` service (see "Installing"). Using the
project's own programs instead:

```sh
cargo build --release
sudo ./data/install-login.sh           # saves the current greetd configuration and installs
sudo ./data/install-login.sh --remove  # restores the previous configuration
```

To see the screen in a window, without installing: `glove --greeter -c "glove-login --fake"` (test
password: `glove`).

## Next steps

The full plan, in phases and steps (one commit each), is in [docs/PLANO.md](docs/PLANO.md) (in
Portuguese), with detailed analyses of each area in [docs/analise/](docs/analise/).

## Neovim with a Nerd Font

The Terminal shows Nerd Font icons by itself if one is installed (it looks for a fallback font only
when a character is missing from the main font). To install one:

```sh
mkdir -p ~/.local/share/fonts && cd ~/.local/share/fonts
curl -LO https://github.com/ryanoasis/nerd-fonts/releases/latest/download/JetBrainsMono.zip
unzip JetBrainsMono.zip && fc-cache -f
```

Then, in Preferences → Terminal, type `JetBrainsMono Nerd Font` (or leave it empty to use the
default font with icons coming from the Nerd Font).

## License

GPL-3.0-or-later (see [LICENSE](LICENSE)). The icon theme is also published separately:
[glove-icons](https://github.com/fabioaacarneiro/glove-icons).
