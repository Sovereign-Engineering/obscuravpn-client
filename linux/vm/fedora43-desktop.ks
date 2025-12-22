url --mirrorlist=http://mirrors.fedoraproject.org/mirrorlist?repo=fedora-43&arch=x86_64

text
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
%end
