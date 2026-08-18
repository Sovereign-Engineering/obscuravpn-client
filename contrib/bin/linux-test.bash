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
      ["almalinux10-desktop"]="almalinux10"
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
      ["fedora44-desktop"]="https://dl.fedoraproject.org/pub/fedora/linux/releases/44/Everything/x86_64/os/"
      ["almalinux10-desktop"]="https://repo.almalinux.org/almalinux/10/BaseOS/x86_64/os/"
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
      ["almalinux10-desktop"]="inst.ks=file:/almalinux10-desktop.ks console=tty0 console=ttyS0"
      ["archlinux-desktop"]="ip=:::::eth0:dhcp net.ifnames=0 archisobasedir=arch archiso_http_srv=https://mirrors.edge.kernel.org/archlinux/iso/latest/ console=ttyS0"
    )
    if [[ ! -v map[${distro}-${flavor}] ]]; then
        die "unknown autoinstall extra-args for ${distro}-${flavor}"
    fi
    echo "${map[${distro}-${flavor}]}"

    declare -A map=(
      ["debian13-desktop"]="./linux/vm/debian-desktop.preseed.cfg"
      ["fedora44-desktop"]="./linux/vm/fedora44-desktop.ks"
      ["almalinux10-desktop"]="./linux/vm/almalinux10-desktop.ks"
    )
    if [[ -v map[${distro}-${flavor}] ]]; then
      echo "--initrd-inject"
      echo "${map[${distro}-${flavor}]}"
    fi
}

function ssh_run() {
  sxx_run ssh -p 2222 user@localhost "$@"
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
    -cpu host \
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

  local repo_url="http://${REPO_IP}:${REPO_PORT}"
  if [[ ${distro} == debian* ]] || [[ ${distro} == ubuntu* ]]; then
    ssh_run curl -fsSLO "${repo_url}/deb/obscura-repository.deb"
    ssh_run sudo apt install -y ./obscura-repository.deb
    ssh_run sudo apt update
  elif [[ ${distro} == fedora* ]] || [[ ${distro} == alma* ]]; then
    ssh_run sudo rpm --import "${repo_url}/rpm/RPM-GPG-KEY-obscura"
    ssh_run sudo dnf install -y "${repo_url}/rpm/obscura-repository.rpm"
  elif [[ ${distro} == archlinux* ]]; then
    ssh_run curl -fsSLO "${repo_url}/arch/obs-keys.asc" -O "${repo_url}/arch/obs-fingerprint.txt"
    ssh_run sudo pacman-key --add obs-keys.asc
    ssh_run "sudo pacman-key --lsign-key \"\$(< obs-fingerprint.txt)\""
    ssh_run "printf '[obscura]\nServer = %s/arch/\$arch\n' '${repo_url}' | sudo tee -a /etc/pacman.conf"
  else
    die "no repository setup for ${distro}"
  fi
}

function install_obscura() {
  local distro=''
  require_args "distro" "$@"

  if [[ ${distro} == debian* ]] || [[ ${distro} == ubuntu* ]]; then
    ssh_run sudo apt install -y obscura
  elif [[ ${distro} == fedora* ]] || [[ ${distro} == alma* ]]; then
    ssh_run sudo dnf install -y obscura
  elif [[ ${distro} == archlinux* ]]; then
    ssh_run sudo pacman -Sy --noconfirm obscura-keyring obscura
    ssh_run sudo systemctl enable --now obscura.service
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
  ssh_run obscura connect
}

function check_if_mullvad() {
  local distro=''
  require_args "distro" "$@"
  local mullvad_check_output
  sleep 1
  for ip_version in 4 6; do
    mullvad_check_output="$(ssh_run curl -sS https://ipv${ip_version}.am.i.mullvad.net/json 2>&1)" || true
    if [[ "${mullvad_check_output}" == *'"mullvad_exit_ip":true'* ]]; then
      echoerr "Mullvad IPv${ip_version} check passed"
    else
      echoerr "Mullvad IPv${ip_version} check failed: ${mullvad_check_output}"
    fi
  done
  collect_debug_bundle --distro "${distro}"
}

function collect_debug_bundle() {
  local distro=''
  require_args "distro" "$@"
  local bundle_path out_path
  out_path="linux/vm/diagnostics/$(date +%Y%m%d-%H%M%S)-${distro}.zip"
  mkdir -p linux/vm/diagnostics
  echoerr "### Creating debug bundle"
  if bundle_path="$(ssh_run obscura debug-bundle linux-test)" && sxx_run scp -P 2222 "user@localhost:${bundle_path}" "${out_path}"; then
    echoerr "### Debug bundle saved to ${out_path}"
  else
    echoerr "### Failed to collect debug bundle"
  fi
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
  check_if_mullvad --distro "${distro}"

  echoerr "### ${distro} ready, click around in the QEMU window."
  echoerr "### Press Ctrl-C to shut the VM down."
  sleep infinity
}

main "$@"
