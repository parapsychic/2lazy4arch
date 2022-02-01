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
- chroot into installed partition
- git clone this repo
- run ./install.sh
- follow on-screen stuff
- reboot
- login to new user
- run postInstall.sh
