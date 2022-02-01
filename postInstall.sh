#!/bin/bash
HOME=/home/parapsychic

echo "Installing drivers, xorg, display manager, alacritty and necessary software"
sudo pacman -S xf86-video-amdgpu nvidia nvidia-utils xorg lightdm lightdm-webkit2-greeter alacritty nitrogen lxsession 

echo "Changing lightdm.conf to use webkit2-greeter"
sudo sed -i '102 i greeter-session=lightdm-webkit2-greeter' /etc/lightdm/lightdm.conf



echo "Enabling lightdm"
systemctl enable lightdm


echo "Installing yay"
git clone https://aur.archlinux.org/yay.git
cd yay/
makepkg -si PKGBUILD
cd

echo "Installing LibreWolf"
yay -S librefox-bin

echo "Installing fonts"
yay -S ttf-ms-fonts

echo "Cloning dot-files"
git clone https://github.com/parapsychic/dot-files.git

echo " === INSTALLING SUCKLESS ==="
echo "Installing DWM"
mkdir -p $HOME/.dwm
cp -r $HOME/dot-files/dwm/* $HOME/.dwm/
cd $HOME/.dwm/
make
sudo make clean install
sudo mkdir -p /usr/share/xsessions
echo "[Desktop Entry]
Encoding=UTF-8
Name=Dwm
Comment=the dynamic window manager
Exec=dwm
Icon=dwm
Type=XSession" | sudo tee /usr/share/xsessions/dwm.desktop



echo "Installing dmenu"
cp -r $HOME/dot-files/dmenu/* $HOME/.dmenu/
cd $HOME/.dmenu/
make
sudo make clean install


echo "Copying dotfiles"
cp $HOME/dot-files/.bashrc $HOME/
mkdir -p .config/nvim
cp -r $HOME/dot-files/nvim/ $HOME/.config/nvim/
cp $HOME/dot-files/autostart.sh $HOME/

echo "Setting up Git"
git config --global user.email "febinkdominic@outlook.com"
git config --global user.name "parapsychic"
echo "Git is not authenticated with Github"

echo "Installing L33T Software"
echo "mpv, htop, lf, neofetch, fzf, lolcat, ueberzug, some fonts"
sudo pacman -S mpv htop lf neofetch fzf lolcat ueberzug ttf-hack ttf-joypixels
yay -S yt-dlp-drop-in ytfzf tabbed otf-manjari 


echo "htop"

todo="TODO: \n## EDIT GRUB WITH OS_PROBER, SAVED and RESOLUTION ##\n## RUN :PlugInstall inside nvim ##\n## Git Personal Access Token ##"
echo -e $todo
echo -e $todo > $HOME/todo.txt
echo "TODO has been saved to $HOME/todo.txt"
echo "Post Install finished successfully (hopefully)"
