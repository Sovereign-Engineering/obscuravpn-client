#!/usr/bin/env bash
set -eux
trap 'pkill -P $$' EXIT

function error() {
  echo "$@" >&2
  kill $$
}

function check_args() {
  if [ "$1" -ne "$2" ]; then
    error "L${BASH_LINENO[0]}: wrong number of function arguments, got $1, expected $2"
  fi
}

function reset() {
  check_args $# 2
  DISTRO=$1
  FLAVOR=$2

  echo "Creating disk image"
  virsh --connect qemu:///session destroy "${DISTRO}-${FLAVOR}" &> /dev/null || true
  qemu-img create -f qcow2 "$(disk_image_path "${DISTRO}" "${FLAVOR}").tmp" 20G

  echo "Downloading ${DISTRO}-${FLAVOR} installation media if necessary"
  download "${DISTRO}" "${FLAVOR}"
  prepare "${DISTRO}" "${FLAVOR}"

  echo "Installing ${DISTRO}-${FLAVOR}"
  mapfile -t AUTOINSTALL_ARGS < <(autoinstall "${DISTRO}" "${FLAVOR}")
  virt-install \
    --connect qemu:///session \
    --transient \
    --name "obs-${DISTRO}-${FLAVOR}" \
    --ram 4096 \
    --vcpus $(($(nproc)-1)) \
    --cpu host-model \
    --disk path="$(disk_image_path "${DISTRO}" "${FLAVOR}").tmp,format=qcow2,bus=virtio" \
    --network user \
    --graphics none \
    --video virtio \
    "${AUTOINSTALL_ARGS[@]}"

    mv "$(disk_image_path "${DISTRO}" "${FLAVOR}").tmp" "$(disk_image_path "${DISTRO}" "${FLAVOR}")"
}

function disk_image_path() {
  check_args $# 2
  DISTRO=$1
  FLAVOR=$2
  echo "./linux/vm/${DISTRO}-${FLAVOR}.qcow2"
}

function download() {
  check_args $# 2
  DISTRO=$1
  FLAVOR=$2
  # Ubuntu doesn't have small desktop or netinstall images, so we need to download the iso
  declare -A map=(
    ["ubuntu24.04-desktop"]="https://releases.ubuntu.com/noble/ubuntu-24.04.3-desktop-amd64.iso"
  )
  if [[ -v map[${DISTRO}-${FLAVOR}] ]]; then
    ISO="./linux/vm/${DISTRO}-${FLAVOR}.iso"
    if [ ! -e "${ISO}" ]; then
      wget "${map[${DISTRO}-${FLAVOR}]}" -O "${ISO}"
    fi
  fi
}

function prepare() {
  check_args $# 2
  DISTRO=$1
  FLAVOR=$2
  # Ubuntu on desktop doesn't support auto install via initrd injected files
  declare -A map=(
    ["ubuntu24.04-desktop"]="x"
    ["archlinux-desktop"]="x"
  )
  if [[ -v map[${DISTRO}-${FLAVOR}] ]]; then
    cloud-localds "./linux/vm/${DISTRO}-${FLAVOR}.seed.iso" "./linux/vm/${DISTRO}-${FLAVOR}-cloud-init/user-data" "./linux/vm/${DISTRO}-${FLAVOR}-cloud-init/meta-data"
  fi
}
function autoinstall() {
    check_args $# 2
    DISTRO=$1
    FLAVOR=$2

    echo "--os-variant"
    declare -A map=(
      ["debian12-desktop"]="debian12"
      ["debian13-desktop"]="debian13"
      ["ubuntu24.04-desktop"]="ubuntu24.04"
      ["fedora43-desktop"]="fedora41"
      ["archlinux-desktop"]="archlinux"
    )
    if [[ ! -v map[${DISTRO}-${FLAVOR}] ]]; then
      error "unknown autoinstall os-variant for ${DISTRO}-${FLAVOR}"
    fi
    echo "${map[${DISTRO}-${FLAVOR}]}"

    echo "--location"
    declare -A map=(
      ["debian12-desktop"]="https://deb.debian.org/debian/dists/bookworm/main/installer-amd64/"
      ["debian13-desktop"]="https://deb.debian.org/debian/dists/trixie/main/installer-amd64/"
      ["ubuntu24.04-desktop"]="./linux/vm/ubuntu24.04-desktop.iso,kernel=casper/vmlinuz,initrd=casper/initrd"
      ["fedora43-desktop"]="https://download.fedoraproject.org/pub/fedora/linux/releases/43/Everything/x86_64/os/"
      ["archlinux-desktop"]="https://mirrors.edge.kernel.org/archlinux/iso/latest/,kernel=arch/boot/x86_64/vmlinuz-linux,initrd=arch/boot/x86_64/initramfs-linux.img"
    )
    if [[ ! -v map[${DISTRO}-${FLAVOR}] ]]; then
      error "unknown autoinstall location for ${DISTRO}-${FLAVOR}"
    fi
    echo "${map[${DISTRO}-${FLAVOR}]}"

    declare -A map=(
      ["ubuntu24.04-desktop"]="x"
      ["archlinux-desktop"]="x"
    )
    if [[ -v map[${DISTRO}-${FLAVOR}] ]]; then
      echo "--disk"
      echo "./linux/vm/${DISTRO}-${FLAVOR}.seed.iso"
    fi

    echo "--extra-args"
    declare -A map=(
      ["debian12-desktop"]="auto=true priority=critical file=/debian-desktop.preseed.cfg console=ttyS0"
      ["debian13-desktop"]="auto=true priority=critical file=/debian-desktop.preseed.cfg console=ttyS0"
      ["ubuntu24.04-desktop"]="autoinstall console=ttyS0"
      ["fedora43-desktop"]="inst.ks=file:/fedora43-desktop.ks console=tty0 console=ttyS0"
      ["archlinux-desktop"]="ip=dhcp archisobasedir=arch archiso_http_srv=https://mirrors.edge.kernel.org/archlinux/iso/latest/ console=ttyS0"
    )
    if [[ ! -v map[${DISTRO}-${FLAVOR}] ]]; then
        error "unknown autoinstall extra-args for ${DISTRO}-${FLAVOR}"
    fi
    echo "${map[${DISTRO}-${FLAVOR}]}"

    declare -A map=(
      ["debian12-desktop"]="./linux/vm/debian12-desktop.preseed.cfg"
      ["debian13-desktop"]="./linux/vm/debian13-desktop.preseed.cfg"
      ["fedora43-desktop"]="./linux/vm/fedora43-desktop.ks"
      ["archlinux-desktop"]="./linux/vm/archlinux-install.sh"
    )
    if [[ -v map[${DISTRO}-${FLAVOR}] ]]; then
      echo "--initrd-inject"
      echo "${map[${DISTRO}-${FLAVOR}]}"
    fi
}

function ssh_run() {
  sxx_run ssh -p 2222 user@localhost "$@"
}

function scp_run() {
  check_args $# 2
  SRC=$1
  DEST=$2
  sxx_run "scp -P 2222" "${SRC}" "user@localhost:${DEST}"
}

function sxx_run() {
  CMD=$1
  shift
  sshpass -p pw "${CMD}" -o ConnectTimeout=1 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR "$@"
}

function start_vm() {
  check_args $# 2
  DISTRO=$1
  FLAVOR=$2

  qemu-system-x86_64 \
    -enable-kvm \
    -m 4G \
    -smp $(($(nproc) - 1)) \
    -drive file="$(disk_image_path "${DISTRO}" "${FLAVOR}"),format=qcow2,if=virtio,snapshot=on" \
    -netdev user,id=n1,hostfwd=tcp::2222-:22 \
    -device virtio-net,netdev=n1 &

  echo "### Started ${DISTRO}-${FLAVOR}, waiting for SSH login"
  until ssh_run exit; do
    sleep 1
  done
  echo "### SSH login on ${DISTRO}-${FLAVOR} successful"
}

function install_package() {
  check_args $# 2
  DISTRO=$1
  FLAVOR=$2

  if [[ ${DISTRO} == debian* ]] || [[ ${DISTRO} == ubuntu* ]]; then
    scp_run ./obscuravpn_0.0.1_amd64.deb /home/user/obscuravpn.deb
    ssh_run sudo dpkg -i /home/user/obscuravpn.deb
  elif [[ ${DISTRO} == fedora* ]] || [[ ${DISTRO} == alma* ]]; then
    scp_run ./obscuravpn-0.0.1-1.x86_64.rpm /home/user/obscuravpn.rpm
    ssh_run sudo dnf install -y /home/user/obscuravpn.rpm
  elif [[ ${DISTRO} == archlinux* ]]; then
    scp_run ./obscuravpn-0.0.1-1-x86_64.pkg.tar.zst /home/user/obscuravpn.zst
    ssh_run sudo pacman --noconfirm -U /home/user/obscuravpn.zst
    ssh_run sudo systemctl enable --now obscuravpn
  else
    error "no package install instructions for this ${DISTRO}"
  fi
}

# shellcheck disable=SC2120
function login_and_connect() {
  check_args $# 0
  # TODO: placeholder until cli login and cli start/status are implemented
  sleep 10
}

# shellcheck disable=SC2120
function check_if_mullvad() {
  check_args $# 0
  for IP_VERSION in 4 6; do
    MULLVAD_CHECK_OUTPUT="$(ssh_run curl -sS https://ipv${IP_VERSION}.am.i.mullvad.net/json)"
    if [[ "${MULLVAD_CHECK_OUTPUT}" == *'"mullvad_exit_ip":true'* ]]; then
      echo "Mullvad IPv${IP_VERSION} check passed"
    else
      error "Mullvad IPv${IP_VERSION} check failed: ${MULLVAD_CHECK_OUTPUT}"
    fi
  done
}

# MAIN
if [ $# -ne 2 ]; then
  error "usage: $0 <account_id> <distro>"
fi
ACCOUNT_ID=$1
DISTRO=$2
FLAVOR="desktop"

if [ ! -f "$(disk_image_path "${DISTRO}" "${FLAVOR}")" ]; then
  reset "${DISTRO}" "${FLAVOR}"
fi

start_vm "${DISTRO}" "${FLAVOR}"

# TODO: remove once cli login is implemented
ssh_run sudo mkdir /var/lib/obscuravpn
ssh_run sudo echo "'{\"account_id\":\"${ACCOUNT_ID}\"}'" ">" /home/user/config.json
ssh_run sudo cp /home/user/config.json /var/lib/obscuravpn

install_package "${DISTRO}" "${FLAVOR}"

login_and_connect
check_if_mullvad

sleep 100000000
