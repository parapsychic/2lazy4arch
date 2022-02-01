#!/bin/bash

echo "Creating Swap partition"
dd if=/dev/zero of=/swapfile bs=1M count=2048 status=progress
chmod 600 /swapfile
mkswap /swapfile
swapon /swapfile
echo /swapfile none  swap defaults 0 0 >> /etc/fstab

echo "Synchronizing Timezones"
ln -sf /usr/share/zoneinfo/Asia/Kolkata /etc/localtime
hwclock --systohc

echo "Generating Locale"
echo en_US.UTF-8 UTF-8 >>  /etc/locale.gen
locale-gen
echo "LANG=en_US.UTF-8" >> /etc/locale.conf

echo "Setting Hostname and hosts"
echo "wonderland" > /etc/hostname
echo -e "127.0.0.1\tlocalhost\n::1\tlocalhost\n127.0.1.1\twonderland.localdomain\twonderland" >> /etc/hosts

echo "Setting up root user"
echo "Enter root password: " 
passwd 

echo "Installing packages"
pacman -S grub efibootmgr os-prober ntfs-3g networkmanager network-manager-applet wireless_tools wpa_supplicant dialog mtools dosfstools base-devel linux-headers bluez bluez-utils pulseaudio-bluetooth alsa-utils cups neovim 

echo "Installing grub"
echo "os-prober is disabled, windows won't be recognized"
echo "run grub-mkconfig again with edited grub file"
grub-install --target=x86_64-efi --efi-directory=/boot --bootloader-id=GRUB
grub-mkconfig -o /boot/grub/grub.cfg

echo "Enabling Services"
systemctl enable NetworkManager
systemctl enable bluetooth

echo "User Management"
useradd -mG wheel parapsychic
echo "Enter user password"
passwd parapsychic 

echo "Adding wheel to sudoers"
visudo -c sudoers

echo "Copying postInstall.sh to /home/parapsychic/"
cp postInstall.sh /home/parapsychic/

echo "Reboot To regular user (parapsychic) and run postInstall.sh"
echo "Install hopefully completed successfully"
