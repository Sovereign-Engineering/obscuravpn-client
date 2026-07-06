#!/usr/bin/env bash
set -eu
trap 'pkill -P $$' EXIT

source contrib/shell/source-require-args.bash

REPO_IP="10.0.2.2"
REPO_PORT=54321

function reset() {
  local distro='' flavor=''
  require_args "distro flavor" "$@"

  echoerr "Creating disk image"
  virsh --connect qemu:///session destroy "obs-${distro}-${flavor}" &> /dev/null || true
  qemu-img create -f qcow2 "$(disk_image_path --distro "${distro}" --flavor "${flavor}").tmp" 20G

  echoerr "Downloading ${distro}-${flavor} installation media if necessary"
  download --distro "${distro}" --flavor "${flavor}"
  prepare --distro "${distro}" --flavor "${flavor}"

  echoerr "Installing ${distro}-${flavor}"
  local autoinstall_out autoinstall_args
  autoinstall_out="$(autoinstall --distro "${distro}" --flavor "${flavor}")"
  mapfile -t autoinstall_args <<<"$autoinstall_out"
  virt-install \
    --connect qemu:///session \
    --transient \
    --name "obs-${distro}-${flavor}" \
    --ram 4096 \
    --vcpus $(($(nproc)-1)) \
    --cpu host-model \
    --disk path="$(disk_image_path --distro "${distro}" --flavor "${flavor}").tmp,format=qcow2,bus=virtio" \
    --network user \
    --graphics none \
    --video virtio \
    "${autoinstall_args[@]}"

    mv "$(disk_image_path --distro "${distro}" --flavor "${flavor}").tmp" "$(disk_image_path --distro "${distro}" --flavor "${flavor}")"
}

function disk_image_path() {
  local distro='' flavor=''
  require_args "distro flavor" "$@"
  echo "./linux/vm/${distro}-${flavor}.qcow2"
}

function download() {
  local distro='' flavor=''
  require_args "distro flavor" "$@"
  # Ubuntu doesn't have small desktop or netinstall images, so we need to download the iso
  declare -A map=(
    ["ubuntu26.04-desktop"]="https://releases.ubuntu.com/26.04/ubuntu-26.04-desktop-amd64.iso"
  )
  if [[ -v map[${distro}-${flavor}] ]]; then
    local iso="./linux/vm/${distro}-${flavor}.iso"
    if [ ! -e "${iso}" ]; then
      wget "${map[${distro}-${flavor}]}" -O "${iso}"
    fi
  fi
}

function prepare() {
  local distro='' flavor=''
  require_args "distro flavor" "$@"
  # Ubuntu on desktop doesn't support auto install via initrd injected files
  declare -A map=(
    ["ubuntu26.04-desktop"]="x"
    ["archlinux-desktop"]="x"
  )
  if [[ -v map[${distro}-${flavor}] ]]; then
    cloud-localds "./linux/vm/${distro}-${flavor}.seed.iso" "./linux/vm/${distro}-${flavor}-cloud-init/user-data" "./linux/vm/${distro}-${flavor}-cloud-init/meta-data"
  fi
}
function autoinstall() {
    local distro='' flavor=''
    require_args "distro flavor" "$@"

    echo "--os-variant"
    declare -A map=(
      ["debian13-desktop"]="debian13"
      ["ubuntu26.04-desktop"]="ubuntu24.04"
      ["fedora44-desktop"]="fedora41"
      ["archlinux-desktop"]="archlinux"
    )
    if [[ ! -v map[${distro}-${flavor}] ]]; then
      die "unknown autoinstall os-variant for ${distro}-${flavor}"
    fi
    echo "${map[${distro}-${flavor}]}"

    echo "--location"
    declare -A map=(
      ["debian13-desktop"]="https://deb.debian.org/debian/dists/trixie/main/installer-amd64/"
      ["ubuntu26.04-desktop"]="./linux/vm/ubuntu26.04-desktop.iso,kernel=casper/vmlinuz,initrd=casper/initrd"
      ["fedora44-desktop"]="https://download.fedoraproject.org/pub/fedora/linux/releases/44/Everything/x86_64/os/"
      ["archlinux-desktop"]="https://mirrors.edge.kernel.org/archlinux/iso/latest/,kernel=arch/boot/x86_64/vmlinuz-linux,initrd=arch/boot/x86_64/initramfs-linux.img"
    )
    if [[ ! -v map[${distro}-${flavor}] ]]; then
      die "unknown autoinstall location for ${distro}-${flavor}"
    fi
    echo "${map[${distro}-${flavor}]}"

    declare -A map=(
      ["ubuntu26.04-desktop"]="x"
      ["archlinux-desktop"]="x"
    )
    if [[ -v map[${distro}-${flavor}] ]]; then
      echo "--disk"
      echo "./linux/vm/${distro}-${flavor}.seed.iso"
    fi

    echo "--extra-args"
    declare -A map=(
      ["debian13-desktop"]="auto=true priority=critical file=/debian-desktop.preseed.cfg console=ttyS0"
      ["ubuntu26.04-desktop"]="autoinstall console=ttyS0"
      ["fedora44-desktop"]="inst.ks=file:/fedora44-desktop.ks console=tty0 console=ttyS0"
      ["archlinux-desktop"]="ip=dhcp net.ifnames=0 archisobasedir=arch archiso_http_srv=https://mirrors.edge.kernel.org/archlinux/iso/latest/ console=ttyS0"
    )
    if [[ ! -v map[${distro}-${flavor}] ]]; then
        die "unknown autoinstall extra-args for ${distro}-${flavor}"
    fi
    echo "${map[${distro}-${flavor}]}"

    declare -A map=(
      ["debian13-desktop"]="./linux/vm/debian-desktop.preseed.cfg"
      ["fedora44-desktop"]="./linux/vm/fedora44-desktop.ks"
    )
    if [[ -v map[${distro}-${flavor}] ]]; then
      echo "--initrd-inject"
      echo "${map[${distro}-${flavor}]}"
    fi
}

function ssh_run() {
  sxx_run ssh -p 2222 user@localhost "$@"
}

function scp_run() {
  local src='' dest=''
  require_args "src dest" "$@"
  sxx_run scp -P 2222 "${src}" "user@localhost:${dest}"
}

function sxx_run() {
  local cmd=$1
  shift
  sshpass -p pw "${cmd}" -o ConnectTimeout=1 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR "$@"
}

function start_vm() {
  local distro='' flavor=''
  require_args "distro flavor" "$@"

  qemu-system-x86_64 \
    -enable-kvm \
    -m 4G \
    -smp $(($(nproc) - 1)) \
    -drive file="$(disk_image_path --distro "${distro}" --flavor "${flavor}"),format=qcow2,if=virtio,snapshot=on" \
    -netdev user,id=n1,hostfwd=tcp::2222-:22 \
    -device virtio-net,netdev=n1 \
    -vga virtio &

  echoerr "### Started ${distro}-${flavor}, waiting for SSH login"
  until ssh_run exit; do
    sleep 1
  done
  echoerr "### SSH login on ${distro}-${flavor} successful"
}

function serve_repo() {
  if [ ! -d result-linux/dist-test ]; then
    die "result-linux/dist-test not found; build the test repo with: ./contrib/bin/linux-build-packages.bash --test"
  fi
  echoerr "### Serving test repo at http://${REPO_IP}:${REPO_PORT}"
  python3 -m http.server "${REPO_PORT}" --bind 0.0.0.0 --directory result-linux/dist-test &
  sleep 1
}

function add_repo() {
  local distro=''
  require_args "distro" "$@"

  if [[ ${distro} == debian* ]] || [[ ${distro} == ubuntu* ]]; then
    local pkgs=(result-linux/dist-test/deb/pool/main/obscura-repository_*.deb)
    scp_run --src "${pkgs[0]}" --dest /home/user/obscura-repository.deb
    ssh_run sudo apt-get install -y /home/user/obscura-repository.deb
    ssh_run sudo apt-get update
  elif [[ ${distro} == fedora* ]] || [[ ${distro} == alma* ]]; then
    local pkgs=(result-linux/dist-test/rpm/x86_64/obscura-repository-*.rpm)
    scp_run --src "${pkgs[0]}" --dest /home/user/obscura-repository.rpm
    ssh_run sudo dnf install -y --nogpgcheck /home/user/obscura-repository.rpm
  elif [[ ${distro} == archlinux* ]]; then
    local pkgs=(result-linux/dist-test/arch/x86_64/obscura-keyring-*.pkg.tar.zst)
    scp_run --src "${pkgs[0]}" --dest /home/user/obscura-keyring.pkg.tar.zst
    ssh_run sudo pacman -U --noconfirm /home/user/obscura-keyring.pkg.tar.zst
    ssh_run "printf '[obscura]\nServer = %s/arch/\$arch\n' 'http://${REPO_IP}:${REPO_PORT}' | sudo tee -a /etc/pacman.conf"
    ssh_run sudo pacman -Sy
  else
    die "no repository setup for ${distro}"
  fi
}

function install_obscura() {
  local distro=''
  require_args "distro" "$@"

  if [[ ${distro} == debian* ]] || [[ ${distro} == ubuntu* ]]; then
    ssh_run sudo apt-get install -y obscura
  elif [[ ${distro} == fedora* ]] || [[ ${distro} == alma* ]]; then
    ssh_run sudo dnf install -y obscura
  elif [[ ${distro} == archlinux* ]]; then
    ssh_run sudo pacman -S --noconfirm obscura
    ssh_run sudo systemctl enable --now obscura
  else
    die "no obscura install for ${distro}"
  fi
  wait_for_service
}

function wait_for_service() {
  echoerr "### Waiting for the obscura service to become active"
  local state
  while true; do
    state="$(ssh_run systemctl show -p ActiveState --value obscura)"
    case "${state}" in
      active) return ;;
      activating) sleep 1 ;;
      *)
        echoerr "### obscura service in unexpected state '${state}'; diagnostics follow"
        ssh_run systemctl status obscura --no-pager --full || true
        ssh_run sudo journalctl -u obscura --no-pager -n 200 || true
        die "obscura service failed to become active (ActiveState=${state})"
        ;;
    esac
  done
}

function setup_and_connect() {
  local account_id=''
  require_args "account_id" "$@"
  ssh_run obscura add-operator user
  ssh_run RUST_LOG=debug obscura ipc-test
  ssh_run obscura login "${account_id}"
  ssh_run obscura start
}

function check_if_mullvad() {
  local mullvad_check_output
  for ip_version in 4 6; do
    mullvad_check_output="$(ssh_run curl -sS https://ipv${ip_version}.am.i.mullvad.net/json)"
    if [[ "${mullvad_check_output}" == *'"mullvad_exit_ip":true'* ]]; then
      echoerr "Mullvad IPv${ip_version} check passed"
    else
      die "Mullvad IPv${ip_version} check failed: ${mullvad_check_output}"
    fi
  done
}

main() {
  local account_id='' distro=''
  require_args "account_id distro" "$@"
  local flavor="desktop"

  if [ ! -f "$(disk_image_path --distro "${distro}" --flavor "${flavor}")" ]; then
    reset --distro "${distro}" --flavor "${flavor}"
  fi

  serve_repo
  start_vm --distro "${distro}" --flavor "${flavor}"

  add_repo --distro "${distro}"
  install_obscura --distro "${distro}"

  setup_and_connect --account_id "${account_id}"
  check_if_mullvad

  echoerr "### ${distro} ready, click around in the QEMU window."
  echoerr "### Press Ctrl-C to shut the VM down."
  sleep infinity
}

main "$@"
