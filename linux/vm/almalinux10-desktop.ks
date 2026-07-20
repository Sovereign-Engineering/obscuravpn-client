url --url=https://repo.almalinux.org/almalinux/10/BaseOS/x86_64/os/
repo --name=AppStream --baseurl=https://repo.almalinux.org/almalinux/10/AppStream/x86_64/os/

text
xconfig --startxonboot
lang en_US.UTF-8
keyboard us
timezone UTC
network --bootproto=dhcp --activate
rootpw pw --plaintext
user --name=user --password=pw --plaintext --groups=wheel
services --enabled=sshd
clearpart --all --initlabel
autopart
reboot

%packages
@^workstation-product-environment
curl
net-tools  # Contains ifconfig and route
openssh-server
%end

%post
echo 'user ALL=(ALL) NOPASSWD: ALL' > /etc/sudoers.d/user
chmod 0440 /etc/sudoers.d/user
dnf install -y epel-release
%end
