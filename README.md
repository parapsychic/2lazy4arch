## Notice:
This project is being rewritten in Rust to be a more general purpose Arch installer launched from the live environment.
I'm not even 10% done with the rewrite, but you can check it out on the branch `rust-rewrite`.

# 2lazy4arch
installing arch really fast, can't be bothered to setup everything. Most I can do is partition disks. Let the computer do the rest. does not follow the best practices, but it works.

### HOW TO USE
First of all, don't.
If you are still reading, you might want to replace all instances of "parapsychic" with your username (unless you want to end up with that username).
Also replace the hostname (wonderland) with whatever you like. There's 2-3 instances of that.
Yes, I could've made these into variables. But I won't. I want anyone other than me using this to go through what I've written. 

- Do base install (base, linux, linux-firmware, ucodes for amd/intel if applicable
- I recommend installing a text editor at this phase in case you want to edit something. The script installs nvim, but I recommend installing some editor at this point.
- Edit the files with your username and hostname.
- Install git
- **generate fstab**
- chroot into installed partition
- git clone this repo
- run ./install.sh
- follow on-screen stuff
- reboot
- login to new user
- run postInstall.sh

### WHAT WILL YOU BE LEFT WITH
A functioning minimal system
- Suckless stuff : DWM and dmenu | no desktop environment
- xorg and lightdm with webkit2 greeter
- Alacritty as terminal emulator (themed)
- NeoVim (plugins with VimPlug) as text editor, remapped a bunch of keys. see config.
- classic necessary programs like htop, youtube-dl
- classic Arch stuff like neofetch and lolcat
- Nitrogen for wallpaper. First time setup necessary.
- a bashrc that's decent
- grub, because why not?
- pacman with color and pacman. because why not?
- dot files include a configuration for i3 with xfce. Requires rofi, feh, and picom


### HOW TO EXTEND
- DWM source in .dwm
- dmenu source in .dmenu
- autostart script starts stuff when dwm is started. It also sets the battery/time/username bar at top. Edit it to start more stuff at startup. Located at $HOME/autostart.sh. Don't forget &
- all other configs in usual .config file.
- I've kept most of the keybindings from original dwm. It's better.
