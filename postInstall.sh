#!/bin/bash
HOME=/home/parapsychic

echo "Installing drivers, xorg, display manager, alacritty and necessary software"
sudo pacman -Syu xf86-video-amdgpu nvidia nvidia-utils xorg alacritty nitrogen lxsession dunst xclip redshift conky

echo "Setting up pacman in style"
sudo sed -i '33 i ILoveCandy' /etc/pacman.conf
sudo sed -i '34 i Color' /etc/pacman.conf

echo "Installing yay"
git clone https://aur.archlinux.org/yay.git
cd yay/
makepkg -si PKGBUILD
cd

echo "Installing LibreWolf"
yay -Syu librefox-bin

echo "Installing fonts"
yay -Syu ttf-ms-fonts

echo "Installing picom-jonaburg"
yay -Syu picom-jonaburg-git

echo "Cloning dot-files"
git clone https://github.com/parapsychic/dot-files.git

echo "Installing startx"
pacman -Syu xorg-xinit
cp $HOME/dot-files/.xinitrc $HOME/

echo " === INSTALLING SUCKLESS ==="
echo "Installing DWM"
cp -r $HOME/dot-files/dwm/ $HOME/.dwm
cd $HOME/.dwm/
make
sudo make clean install
sudo mkdir -p /usr/share/xsessions
echo "[Desktop Entry]
Encoding=UTF-8
Name=Dwm
Comment=the dynamic window manager
Exec=/usr/local/bin/dwm
Icon=dwm
Type=XSession" | sudo tee /usr/share/xsessions/dwm.desktop



echo "Installing dmenu"
cp -r $HOME/dot-files/dmenu/ $HOME/.dmenu
cd $HOME/.dmenu/
make
sudo make clean install


echo "Copying dotfiles"
cp $HOME/dot-files/.bashrc $HOME/
cp -r $HOME/dot-files/nvim/ $HOME/.config/nvim
cp $HOME/dot-files/autostart.sh $HOME/
cp -r $HOME/dot-files/dunst/ $HOME/.config/dunst
cp -r $HOME/dot-files/conky/ $HOME/.config/conky
cp -r $HOME/dot-files/alacritty $HOME/.config/alacritty


mkdir -p $HOME/.bin 
cp -r $HOME/dot-files/lf/ $HOME/.config/lf
cp -r $HOME/dot-files/.bin/ $HOME/.bin

echo "Setting up Git"
git config --global user.email "febinkdominic@outlook.com"
git config --global user.name "parapsychic"
echo "Git is not authenticated with Github"

echo "Installing L33T Software"
echo "mpv, htop, lf, neofetch, fzf, lolcat, ueberzug, some fonts"
sudo pacman -S mpv htop neofetch fzf lolcat ueberzug ttf-hack ttf-joypixels brightnessctl
yay -Syu yt-dlp-drop-in ytfzf lf tabbed otf-manjari 

echo "Setting up touchpad"
sudo mkdir -p /etc/X11/xorg.conf.d
sudo tee <<'EOF' /etc/X11/xorg.conf.d/90-touchpad.conf 1> /dev/null

Section "InputClass"
    Identifier "touchpad"
    MatchIsTouchpad "on"
    Driver "libinput"
    Option "Tapping" "on"
    Option "NaturalScrolling" "on"
    Option "ScrollMethod" "twofinger"
    Option "TappingDrag" "on"
    Option "DisableWhileTyping" "on"
EndSection
EOF


echo "Configuration complete"

todo="TODO: \n## EDIT GRUB WITH OS_PROBER, SAVED and RESOLUTION ##\n## RUN :PlugInstall inside nvim ##\n## Git Personal Access Token ##"
echo -e $todo
echo -e $todo > $HOME/todo.txt
echo "TODO has been saved to $HOME/todo.txt"
echo "Post Install finished successfully (hopefully)"
