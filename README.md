# Organizador PC

🇬🇧 English · 🇧🇷 [Português](README.pt-BR.md)

Watches a folder (e.g. `~/Downloads`) and moves every new item into the folder
of its category, following rules in a plain text file. It also organizes what
was already there when the program starts. It works offline, with rules only
(no AI, no network).

Linux only (it uses inotify and a systemd user service).

## Installation

```bash
./instalar.sh
```

The script builds the program (needs `cargo`), installs the binary into
`~/.local/bin`, creates `~/.config/organizador-pc/config.txt` from the example
config in your language, shows a dry run and, if you confirm, enables the
systemd user service, which starts with your session. To start at boot, before
login: `loginctl enable-linger $USER`.

Uninstall: `./instalar.sh --uninstall` (the config is kept).

### Moving it to another PC

```bash
./empacotar.sh
```

This creates two equivalent options in `dist/`: the `organizador-pc/` folder
and the `organizador-pc.tar.gz` archive. The package carries the compiled
executable (the other PC does not need Rust) and your current config. On the
other PC:

```bash
tar xzf organizador-pc.tar.gz   # only if you took the .tar.gz
cd organizador-pc
./instalar.sh
```

If the other PC already has a config installed, it is kept. The executable
runs on distros with glibc 2.34 or newer (Ubuntu 22.04+, Debian 12+,
Fedora 35+). For a fully static executable, install the musl target (on Arch:
`pacman -S rust-musl`) before running `empacotar.sh`.

Use `~` and `$XDG_..._DIR` in the config, never `/home/your-user`: that way the
same file works with any user and any system language.

## Configuration

See the comments in `config.example.txt` (Portuguese version:
`config.exemplo.txt`). Keys work in English or Portuguese (`SOURCE_DIR` =
`PASTA_FONTE`, `OTHERS_DIR` = `PASTA_OUTROS`, `WAIT_SECONDS` =
`ESPERA_SEGUNDOS`, `NOTIFICATIONS` = `NOTIFICACOES`, `CATEGORIES` =
`CATEGORIAS`, `words` = `palavras`). Summary:

```text
SOURCE_DIR: $XDG_DOWNLOAD_DIR
OTHERS_DIR: $XDG_DOWNLOAD_DIR/OTHERS
WAIT_SECONDS: 5
NOTIFICATIONS: yes
CATEGORIES:
1. VIDEOS: $XDG_VIDEOS_DIR | ext: mp4, mkv
2. INVOICES: ~/Documents/Invoices | ext: pdf | words: invoice, receipt
3. BOOKS: ~/Books | ext: pdf, epub
```

Destination folders that do not exist are created.

**Config changes apply as soon as the file is saved**, with no restart:

- the new rules take effect and the watched folder is reorganized with them;
- what was in `OTHERS_DIR` and now fits a category leaves it (e.g. you add a
  category for `.json` → the `.json` files in OTHERS move to it);
- if `SOURCE_DIR` changes, the program starts watching the new folder;
- if the saved config has an error, the previous configuration stays in effect
  and a notification shows the offending line. If the program starts with an
  invalid config, it waits for the file to be fixed.

Items already moved into category folders are left alone: if you change a
category's destination, only the next items go to the new place.

## How classification works

**Loose files** in the watched folder:

| category rules  | matches when                               | priority  |
|-----------------|--------------------------------------------|-----------|
| ext + words     | extension **and** at least one word        | 3 (highest) |
| words only      | at least one word in the name              | 2         |
| ext only        | extension                                  | 1         |

Ties: the category with the most matched words wins, then the one that comes
first in the config. Words ignore case and accents. Words of up to 3 letters
(`nfe`, `rm`) must appear as a separate word, so `anfe.pdf` does not count as
`nfe`. If the extension is in no category (or the file has none), the type is
detected from the content (e.g. a PDF without `.pdf`). With no matching rule,
the file goes to `OTHERS_DIR`.

**Subfolders** are moved whole, never taken apart. Each file inside is
classified (folder names also count as words) and the folder goes to the
category of more than half of its files. With no majority (e.g. extracted
source code), it goes to `OTHERS_DIR`.

**When an item is moved:** only after it has gone `WAIT_SECONDS` without
changing and with no temporary download files (`.part`, `.crdownload`, ...).
Empty folders stay where they are. Nothing is overwritten: if the name already
exists at the destination, the item becomes `name (1).ext`.

**Notifications:** after each batch of moved items a desktop notification
appears (e.g. "3 items organized", listing up to 5). Errors come first and are
urgent. It uses `notify-send` (package `libnotify`); without it the program
works normally, just without notifications. To turn them off:
`NOTIFICATIONS: no`.

**Ignored:** hidden items (starting with `.`), the config itself, destination
folders that live inside the watched folder, and symbolic links.

## Manual use

```bash
organizador-pc --dry-run   # show what it would do, move nothing (--simular)
organizador-pc --once      # organize what is already there and exit (--uma-vez)
organizador-pc -c other-config.txt
journalctl --user -u organizador-pc -f   # follow the service
```

## Language

Program messages (terminal and notifications) follow the system language:
Portuguese if `LC_ALL`, `LC_MESSAGES` or `LANG` starts with `pt`, English
otherwise. `instalar.sh` and `empacotar.sh` do the same.

## Development

```bash
cargo test
cargo build --release
```

Code in `src/`: `config.rs` (config parsing), `classificador.rs` (rules),
`mover.rs` (move without overwriting, even across disks), `texto.rs` (name
normalization), `notificacao.rs` (desktop notices), `i18n.rs` (message
language) and `main.rs` (watching).

Identifiers and code comments are in Portuguese. See
[CONTRIBUTING.md](CONTRIBUTING.md) to contribute.

## License

[MIT](LICENSE)
