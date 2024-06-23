# 2Lazy4Arch: Installing Arch Really Fast
A dead simple, fast and opinionated Arch Linux Installer, written in Rust.

## What to Expect?
- File partitioning using `cfdisk`.
- Reflector to sync pacman mirrors.
- (1,2,4,8,16,32, 64) GB Swapfile instead of swap partition.
- Yay as AUR package manager.
- Gnome/KDE/Hyprland as Desktop Environment / Window Manager options
- Sudo/Doas
- Grub/SystemD Boot as Bootloader
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
pulseaudio-bluetooth
alsa-utils
cups
  ```

## How To Use?
This is a two-part installation process. 

The first part installs till a bootable system. If this is all that you need, complete the part one of this guide.

For the second part, you might need to set up some files.

### Part 1: Installing the base system
The base installer partitions and mounts the filesystem, installs important packages and sets up the users and hostname.
To run it as it is, use curl to download the release and run it. Replace the version with the release tag.

```sh
# curl -L https://github.com/parapsychic/2lazy4arch/releases/download/{release}/2lazy4arch --output 2lazy4arch
#eg:
curl -L https://github.com/parapsychic/2lazy4arch/releases/download/v2.0.0/2lazy4arch --output 2lazy4arch
./2lazy4arch
```
Follow the onscreen instructions.
The TUI is intuitive and also supports vim-style `jk` movements.

Now that installation is successful, you should see either one of the following messages
```
# ERROR:
Failed to get the executable name from arguments.

# ERROR:
Failed to copy the installer.
Please copy the file manually to /some/path

# SUCCESSFUL:
Successfully copied the executable to /some/path. 
Please run the installer after rebooting to the installed system.
Installer completed successfully.

```
Dont worry about these error messages if you don't intend to follow step 2.

Your installation is successful.

If you don't intend to follow step 2 of this installation, you can safely reboot.

Otherwise, continue with step 2.

### Part 2: Post Installation
If you got the success message from last step, an installer file will be present in your `home` folder.
If not, now is your chance to copy the 2Lazy4Arch executable to your `home` folder.
```sh
cp 2lazy4arch /mnt/home/{username}/installer
```
> If you accidentally rebooted in the last step, just download the installer again
> ```sh
> curl -L https://github.com/parapsychic/2lazy4arch/releases/download/v2.0.0/2lazy4arch --output 2lazy4arch
> ```

Before we begin, we should create two files. 

Name them whatever you like. For this example, I'll be calling the `packages.txt` and `aur_packages.txt`.

These files expect valid package names separated by a newline.

Packages in `packages.txt` will be installed using `pacman`.

Packages in `aur_packages.txt` will be installed using `yay`.

Start installation by running the installer file.
```sh
./installer
```
This has no TUI as there were some hiccups along the way with the terminal input/output piping, so this will be completely CLI based.
When prompted, enter the relative path to the packages files.
Keep an eye-out for prompts to enter the password.
After completing the installer will show a message: "Installation has finished. Enjoy!".
See [How To Extend?](how-to-extend?) to know how to run post-install hooks/scripts

#### [Note to me] ParaPsychic Mode
To run my specific settings, run installer after base installation with the `parapsychic-mode` argument.


#### Compiling
Install rust by following this [guide](https://www.rust-lang.org/learn/get-started).
Then, clone this repo and compile it.
```sh
git clone https://github.com/parapsychic/2lazy4arch.git
cd 2lazy4arch
cargo build --release
```
The compiled binary will be at `target/release/toolazy4arch`. The naming is different as rust does not allow first character to be a digit.


## How To Extend?
### Packages
2Lazy4Arch expects you to make some files before running the post installer. Refer to the [How-To-Use?](#how-to-use?) section to learn more.

### Ricing
To run your own ricing scripts after installation:
Go to `installer/src/utils` and change the RICE_SCRIPT_URL
The script is downloaded using `curl`, so be sure to host it somewhere.
After that, run the compile and run the program with the argument `parapsychic-mode`.
```sh
./2lazy4arch parapsychic-mode
```
You can change the argument name, but I made this specifically for running my scripts.

## Screenshots:
![2024-06-24_00-50](https://github.com/parapsychic/2lazy4arch/assets/63157522/d3b8e8b0-4509-47f1-8a3f-7d64ced6eedb)

![2024-06-24_00-51](https://github.com/parapsychic/2lazy4arch/assets/63157522/e391206a-e384-4dd2-9ff6-7c0d85d9b845)

![2024-06-24_00-52_1](https://github.com/parapsychic/2lazy4arch/assets/63157522/4abdc61d-c80e-4d6c-a233-1ad944b5f1ec)

![2024-06-24_00![2024-06-24_00-53](https://github.com/parapsychic/2lazy4arch/assets/63157522/ec38c16b-7866-49b0-aa6f-c33d8e418875)

-52](https://github.com/parapsychic/2lazy4arch/assets/63157522/bee3d071-2ed6-4891-ba19-3b4655b42322)
![2024-06-24_00-56](https://github.com/parapsychic/2lazy4arch/assets/63157522/16601791-0617-4647-bde0-2a4d73f556c6)



