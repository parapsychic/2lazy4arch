# 2Lazy4Arch: Installing Arch Really Fast
A dead simple, fast and opinionated Arch Linux Installer, written in Rust.

## What to Expect?
- A step-by-step TUI: partitioning with `cfdisk`, then mount points, mirrors, swap, timezone, locale, users, bootloader, drivers, desktop and browser, then a review screen before anything is touched.
- Works on Intel and AMD CPUs (microcode is picked automatically) and on Intel, AMD and NVIDIA GPUs, including hybrid laptops.
- Asks whether you want proprietary drivers:
  - NVIDIA: `nvidia-open` (GTX 16xx / RTX and newer), legacy `nvidia-580xx` (GTX 9xx / 10xx, AUR) or `nouveau`. Hybrid laptops also get `nvidia-prime` (`prime-run`).
  - AMD: Mesa, or Mesa + AMDGPU PRO (proprietary Vulkan and AMF, AUR).
  - Intel: always Mesa (`vulkan-intel`, `intel-media-driver`).
- Desktop / window manager, each booting to a login screen:
  DWM (Xorg, built from suckless git with dmenu and st), Hyprland (Wayland), KDE Plasma, GNOME, Xfce, LXDE, or none.
- Browser: Firefox, LibreWolf, Chromium, Vivaldi, or from the AUR: Brave, Zen, Google Chrome.
- Reflector to rank pacman mirrors.
- A swap file (none, 1 to 64 GB) instead of a swap partition.
- Sudo/Doas, Grub (with os-prober for dual boot)/systemd-boot.
- Yay as AUR helper (part 2).
- UEFI only. The installer refuses to start the install when booted in BIOS mode.
- The following programs:
```
base
linux
linux-firmware
intel-ucode/amd-ucode (if your processor is detected)
neovim
reflector
efibootmgr
os-prober
ntfs-3g
networkmanager
network-manager-applet
wireless_tools
wpa_supplicant
dialog
mtools
dosfstools
base-devel
linux-headers
bluez
bluez-utils
pipewire
pipewire-pulse
pipewire-jack
pipewire-alsa
wireplumber
alsa-utils
git
cups
mesa + your GPU drivers
```

## How To Use?
This is a two-part installation process.

Part 1 installs a bootable system with your drivers, desktop and browser. If you picked nothing from the AUR, that's all you need.

Part 2 runs after the first boot. It sets up yay, installs the AUR packages you picked in part 1, and anything listed in your own package files.

### Part 1: Installing the system
Boot the Arch ISO in UEFI mode, connect to the internet, then download a release and run it. Replace the version with the release tag.

```sh
# curl -L https://github.com/parapsychic/2lazy4arch/releases/download/{release}/2lazy4arch \
#  --output 2lazy4arch
#eg:
curl -L https://github.com/parapsychic/2lazy4arch/releases/download/v2.0.0/2lazy4arch \
 --output 2lazy4arch

chmod +x 2lazy4arch

./2lazy4arch
```

Follow the steps. The sidebar shows where you are and what you picked.
- `↑↓` (or `jk` on short lists) to move, `enter` to pick, `esc` to go back, `ctrl+c` to quit.
- Long lists (mirrors, timezones, locales) filter as you type.
- Nothing is formatted until you press `y` on the review screen. The exception is `cfdisk`, which writes partition changes when you save in it.

Before touching the disks, it ranks mirrors and checks that every package it's about to install exists in the repos. Anything missing is listed, and you can stop there with nothing changed or continue without it.

When it's done, it prints `Installation finished.` and copies itself to `/usr/local/bin/2lazy4arch` in the new system. If it fails, the error is printed and every command it ran is in `shell_log.txt`.

Reboot.

### Part 2: Post Installation
Log in as your user (not root) and run:
```sh
2lazy4arch
```
It shows the AUR packages queued from part 1, then asks for two optional files (press enter to skip either):
- a package list installed with `pacman`
- a package list installed with `yay`

Package lists have one package name per line; blank lines and `#` comments are ignored. Names that aren't in the repos (or, for the yay list, the AUR either) are skipped instead of failing the whole install, and listed at the end. See the [example files](https://github.com/parapsychic/2lazy4arch/tree/main/examples).

Keep an eye out for sudo password prompts. When everything worked it prints `Installation has finished. Enjoy!`. Running it again is safe: anything already installed is skipped.

#### [Note to me] ParaPsychic Mode
To run my specific settings (dotfiles, my dwm/dmenu builds, multilib, pacman candy, touchpad config), run part 2 with the `parapsychic-mode` argument:
```sh
2lazy4arch parapsychic-mode
```

#### Compiling
Install rust by following this [guide](https://www.rust-lang.org/learn/get-started).

Then, clone this repo and compile it.
```sh
git clone https://github.com/parapsychic/2lazy4arch.git
cd 2lazy4arch
cargo build --release
cargo test
```
The compiled binary will be at `target/release/toolazy4arch`. The naming is different as rust does not allow first character to be a digit.


## How To Extend?
### Packages
Part 2 takes your own package lists. Refer to the [How-To-Use?](#how-to-use) section to learn more.

### Drivers, desktops, browsers
They all map to packages in `Config::packages()` in [installer/src/config.rs](installer/src/config.rs). The choices shown in the TUI live in [tui/src/app.rs](tui/src/app.rs).

### Ricing
`parapsychic-mode` is `PostInstall::misc_options` in [installer/src/post_install.rs](installer/src/post_install.rs). To rice with your own dotfiles, change `DOTFILES_REPO`, `GIT_NAME` and `GIT_EMAIL` at the top of that file and edit the steps, then [compile](#compiling).

## Problems?
It Just Works<sup>TM</sup>  

<img src="https://yt3.ggpht.com/a/AATXAJxuZBNfke48M_7TcSsN9iMtJmaE1JTNVVfEeg=s900-c-k-c0xffffffff-no-rj-mo" target="_blank" rel="noopener"  height=100px >

But in case it doesn't, [click here](https://newfastuff.com/wp-content/uploads/2019/05/5p3oYv1.png) or open an issue and cross your fingers.

Built with ❤️ and Rust

<sup>Hire me Bethesda, I'll work for minimum wage.</sup>

<a href="https://youtu.be/Eweu-mHzmq4?si=pAnmXEZV0725b7rS" target="_blank" rel="noopener"><img src="https://raw.githubusercontent.com/parapsychic/ishowoff/main/.readme_images/hong.png" height=100px></a>


🫰 I'll probably switch to Nix OS after this...
