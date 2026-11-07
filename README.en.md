# Glove

*Leia em português: [README.md](README.md)*

A lightweight Linux desktop written in Rust, with a look inspired by the "gel" era of the
2000s: translucent buttons, fine pinstripes, brushed metal, soft shadows and a magnifying
Dock. All artwork is drawn in code (no image files) and is original: Mac OS X Tiger is the
style reference, never copied.

The interface speaks **Portuguese and English** (it follows the system language) and new
languages are added with a `.po` file — see [Languages](#languages).

## Current state

Look and behavior baseline: **Mac OS X 10.4 Tiger** (plus a Pinstripes theme for the
10.0–10.2 look). Phases 1 to 9 of the [plan](docs/PLANO.md) (in Portuguese) are done; phase 10 (beyond the era) is under way.

**System (the `glove` compositor)**

- Its own Wayland compositor (Smithay), running as a **full session** (DRM/KMS + libinput) or
  **inside a window**
- **Multiple displays**: extend, mirror or main only; resolution and scale per screen; sharp on
  HiDPI and fractional scaling
- **X11 apps** through Xwayland, with Glove's title bar
- Global menu bar: main menu (Recent Items, Force Quit, Sleep, Shut Down…), the app menu, **each
  app's own menus**, submenus and keyboard navigation; Wi-Fi, volume, battery and display extras,
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
- **All Apps** (F4 or the Dock): every installed app full screen, in pages, with search and a
  Utilities folder
- **Desktops** (Ctrl+Super+arrows; with Shift the window comes along), also in a strip in Exposé,
  where windows can be dragged from one desktop to another
- **Quick Search** (Super+Space): installed apps, Preferences panes and files
- **Screenshots** of the whole screen, an area or a window, on any display
- Force Quit that detects unresponsive apps; screen lock (PAM), screen blanking, laptop lid,
  volume and brightness keys
- Accessibility: zoom, inverted colors, large cursor
- Drag and drop between apps, text and file clipboard
- Glove's Open/Save sheet also for other apps (FileChooser portal: Firefox, Chromium…)
- **Light** (default, Tiger), **Pinstripes**, **Metal** and **Graphite** themes
- Redraws only what changed: 0% CPU when idle

**Session and login**

- Its own login screen on greetd (`glove-login`), with session choice; a code-drawn Plymouth
  splash; first-run assistant; Shut Down/Restart with countdown

**Apps (all built with glove-kit and original artwork)**

| App | What it does |
|---|---|
| Files | The Tiger file browser: metal, sidebar, columns/list/icons, preview, copy/move with progress, Trash, Get Info, smart folders, compress, eject, color labels, Open With, Connect to Server (smb, sftp, ftp, webdav) and Network |
| Preferences | "Show All" grid with the era's panes (Appearance, Desktop & Screen Saver, Dock, Displays, Sound, Network, Power Saving, Keyboard, Mouse, Trackpad, Date & Time, Accounts, International, Startup Items, Accessibility, Terminal…) |
| Terminal | Alacritty core: truecolor, nvim/htop, mouse, OSC 52, themes, Nerd Font |
| Text Editor | Ruler, alignment, Find/Replace, undo, unsaved-changes warning |
| Viewer | Pictures (PNG, JPEG, WebP) and PDF, thumbnails, zoom, rotate |
| Browser | Opens the browser that fits the machine (NetSurf with little RAM, Firefox above) |
| Mail | IMAP and SMTP with TLS, mailboxes, compose, reply and forward |
| Calendar and Contacts | Month grid with events (.ics) and cards (vCard) |
| Music and Video | Light library and player (pure Rust); video through mpv with its own controls |
| Notes | Colored sticky notes, all in one process |
| Dictionary | Free StarDict and DICT dictionaries |
| Font Viewer and Characters | View, install and disable fonts; character map |
| Passwords | Passwords in the system keychain: view, copy, delete, lock and change the keychain password |
| Network Tools | Interface info, Ping, Lookup, Traceroute, Whois, Finger and port scan |
| System Monitor, Logs, System Information, Disks | Processes and graphs, journal logs, hardware, disks and USB drives |
| Search, Screenshot, Archive Manager | Find files, capture the screen, compress/expand |
| Calculator, Clock, Chess, Help, About Glove | Scientific calculator, clock in the Dock, chess with 3 levels, searchable help |

## Lightness

All 32 apps are one executable (`glove-apps`; `glove-files`, `glove-prefs`… are links to it),
so their code is shared in memory and on disk (166 MB → 44 MB), and font files are mapped
instead of copied. With Files, Preferences and the welcome window open, the apps use about
26 MB together. Idle, Glove uses 0.1% of one CPU core. Most of the compositor's memory is the
Mesa OpenGL driver, a fixed cost of any accelerated compositor. Details (in Portuguese):
[docs/MEMORIA.md](docs/MEMORIA.md).

## Running

The apps' library ([glove-ui](https://github.com/fabioaacarneiro/glove-ui)) is a git submodule
in `ui/`: clone with `git clone --recursive` (or run `git submodule update --init` afterwards).

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

## Development session

```sh
cargo build --release
./instalar.sh                         # step by step: session, login screen, Plymouth
./target/release/glove --monitores    # shows the display, resolution and scale the session will use
```

`./instalar.sh` points the session at the project's own programs, so rebuilding updates it.
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
to ship it, add it to `EMBUTIDOS` in `crates/i18n/src/lib.rs`. `cargo test -p glove-i18n`
fails if any marked text has no English translation, and `tools/i18n.py` helps mark new texts.

## Configuration

`~/.config/glove/config.toml` (created on first run). Preferences edits this file and Glove
reloads it when it changes.

## Layout

```
ui/                glove-ui submodule: glove-ui (toolkit), glove-paint (drawing, icons, themes),
                   glove-config (shared settings) and glove-i18n (translations)
crates/compositor  the compositor (`glove`)
crates/apps        `glove-apps`: one executable for every app
crates/portal      `glove-portal` (FileChooser portal) and `glove-escolher`
crates/login       login screen (`glove-login`, on greetd; `--fake` to try it)
crates/arte        `glove-arte`: Plymouth theme and the "Glove" icon theme
crates/…           one folder per app (files, prefs, term, editor, viewer, correio, agenda…)
data/              session, greetd, Plymouth, portals and installers
tools/i18n.py      helper to mark texts for translation
```

## License

GPL-3.0-or-later (see [LICENSE](LICENSE)). The icon theme is also published separately:
[glove-icons](https://github.com/fabioaacarneiro/glove-icons).
