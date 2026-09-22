#!/usr/bin/env bash

set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
work_dir=$(mktemp -d --tmpdir plank-display-test.XXXXXX)
cleanup() {
  rm -rf -- "$work_dir"
}
trap cleanup EXIT

config_file="${work_dir}/plank-host.conf"
output_file="${work_dir}/xorg.conf.d/99-plank-headless.conf"
edid_dir="${work_dir}/display"
fake_bin="${work_dir}/bin"
mkdir -p "$fake_bin"
python3 "$repo_dir/packaging/host/linux/display/generate-virtual-edids.py" "$edid_dir" >/dev/null

[[ $(find "$edid_dir" -maxdepth 1 -type f -name '*.edid' | wc -l) -eq 4 ]]
for index in 1 2 3 4; do
  [[ $(wc -c <"${edid_dir}/virtual-${index}.edid") -eq 384 ]]
  # Distinct identities: product code, serial and "Display N".
  [[ $(od -An -j 10 -N 2 -tx1 "${edid_dir}/virtual-${index}.edid" | tr -d '[:space:]') == "0${index}53" ]]
  display_name=$(printf 'Display %d\n   ' "$index" | od -An -tx1 | tr -d '[:space:]')
  [[ $(od -An -j 113 -N 13 -tx1 "${edid_dir}/virtual-${index}.edid" | tr -d '[:space:]') == "$display_name" ]]
done
# The first two EDIDs keep the qualified pool byte for byte.
[[ $(sha256sum "${edid_dir}/virtual-1.edid" | cut -d' ' -f1) == 693b82c65a705489af536e3877463fa599c85333b0e7a669ce679a6f526ca6f3 ]]
[[ $(sha256sum "${edid_dir}/virtual-2.edid" | cut -d' ' -f1) == 38d0b24aa59ef722f6d90fb19a8f5fef9ebf2e3212f1b3055414097ac6d98167 ]]
[[ ! -e ${edid_dir}/virtual-1-1920x1080.edid ]]
displayid_headers=$(for block_offset in 128 256; do
  od -An -j "$block_offset" -N 8 -tx1 "${edid_dir}/virtual-1.edid" | tr -d '[:space:]'
done)
[[ $displayid_headers == '70136703010300647013670000030064' ]]
for block_offset in 0 128 256; do
  checksum=$(od -An -j "$block_offset" -N 128 -tu1 "${edid_dir}/virtual-1.edid" |
    awk '{ for (field = 1; field <= NF; ++field) sum += $field } END { print sum % 256 }')
  [[ $checksum -eq 0 ]]
done
[[ $(od -An -j 8 -N 2 -tx1 "${edid_dir}/virtual-1.edid" | tr -d '[:space:]') == '418b' ]]
[[ $(od -An -j 113 -N 13 -tx1 "${edid_dir}/virtual-1.edid" | tr -d '[:space:]') == '446973706c617920310a202020' ]]

run_prepare() {
  PATH="${fake_bin}:${PATH}" \
    "$repo_dir/packaging/host/linux/bin/plank-display-prepare" \
      --config "$config_file" --output "$output_file" --edid-dir "$edid_dir"
}

run_requested_prepare() {
  PATH="${fake_bin}:${PATH}" \
    "$repo_dir/packaging/host/linux/bin/plank-display-prepare" \
      --config "$config_file" --output "$output_file" --edid-dir "$edid_dir" \
      "$@"
}

cat >"${fake_bin}/systemctl" <<'EOF'
#!/usr/bin/env bash
exit 1
EOF
chmod 0755 "${fake_bin}/systemctl"

printf '[display]\nstartup_layout = physical\n' >"$config_file"
run_prepare >/dev/null
[[ ! -e $output_file ]]
if run_requested_prepare --layout single --mode-1 3840x2160 >/dev/null 2>&1; then
  echo "a virtual transition bypassed the physical-display policy" >&2
  exit 1
fi

printf '[display]\nstartup_layout = virtual\n' >"$config_file"
run_prepare >/dev/null
grep -Fq 'Option "ConnectedMonitor" "DFP-0, DFP-2"' "$output_file"
grep -Fq 'DFP-0: 1920x1080 +0+0, DFP-2: NULL' "$output_file"
grep -Fq 'virtual-1.edid' "$output_file"
grep -Fq 'virtual-2.edid' "$output_file"
grep -Fq 'Virtual 8192 2160' "$output_file"
if grep -Fq 'AllowNonEdidModes' "$output_file"; then
  echo "generated Xorg overlay permits a non-EDID mode" >&2
  exit 1
fi

run_requested_prepare --layout single --mode-1 2560x1600 >/dev/null
grep -Fq 'DFP-0: 2560x1600 +0+0, DFP-2: NULL' "$output_file"
grep -Fq 'virtual-1.edid' "$output_file"
grep -Fq 'startup_layout = virtual' "$config_file"

run_requested_prepare --layout single --mode-1 5120x2160 >/dev/null
grep -Fq 'DFP-0: 5120x2160 +0+0, DFP-2: NULL' "$output_file"

if run_requested_prepare --layout dual-horizontal --mode-1 4096x2160 >/dev/null 2>&1; then
  echo "a dual transition without mode 2 was accepted" >&2
  exit 1
fi
run_requested_prepare \
  --layout dual-horizontal --mode-1 4096x2160 --mode-2 1024x2160 >/dev/null
grep -Fq 'DFP-0: 4096x2160 +0+0, DFP-2: 1024x2160 +4096+0' "$output_file"
grep -Fq 'virtual-2.edid' "$output_file"

run_requested_prepare --layout single --mode-1 3024x1890 >/dev/null
grep -Fq 'DFP-0: 3024x1890 +0+0, DFP-2: NULL' "$output_file"
grep -Fq 'Virtual 8192 2160' "$output_file"
run_requested_prepare \
  --layout dual-horizontal --mode-1 3024x1890 --mode-2 5120x2160 >/dev/null
grep -Fq 'DFP-0: 3024x1890 +0+0, DFP-2: 5120x2160 +3024+0' "$output_file"
# The panel size is not a mode: fullscreen stops at the camera housing.
if run_requested_prepare --layout single --mode-1 3024x1964 >/dev/null 2>&1; then
  echo "the unqualified 3024x1964 panel size was accepted" >&2
  exit 1
fi

previous_hash=$(sha256sum "$output_file")
printf '[display]\nstartup_layout = three\n' >"$config_file"
if run_prepare >/dev/null 2>&1; then
  echo "an invalid startup display policy was accepted" >&2
  exit 1
fi
[[ $(sha256sum "$output_file") == "$previous_hash" ]]

printf '[display]\nstartup_layout = virtual\nvirtual_mode_1 = 3840x2160\n' >"$config_file"
if run_prepare >/dev/null 2>&1; then
  echo "a removed administrator virtual-mode option was accepted" >&2
  exit 1
fi
[[ $(sha256sum "$output_file") == "$previous_hash" ]]

printf '[display]\nstartup_layout = virtual\n' >"$config_file"
if run_requested_prepare --layout single --mode-1 1280x720 >/dev/null 2>&1; then
  echo "a removed virtual mode was accepted" >&2
  exit 1
fi

cat >"${fake_bin}/systemctl" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod 0755 "${fake_bin}/systemctl"
if run_prepare >/dev/null 2>&1; then
  echo "an active display manager did not block a topology change" >&2
  exit 1
fi
[[ $(sha256sum "$output_file") == "$previous_hash" ]]

cat >"${fake_bin}/systemctl" <<'EOF'
#!/usr/bin/env bash
exit 1
EOF
chmod 0755 "${fake_bin}/systemctl"
printf '[display]\nstartup_layout = physical\n' >"$config_file"
run_prepare >/dev/null
[[ ! -e $output_file ]]

mkdir -p "${output_file%/*}"
printf '# owned by somebody else\n' >"$output_file"
if run_prepare >/dev/null 2>&1; then
  echo "a foreign Xorg configuration was removed" >&2
  exit 1
fi
grep -Fxq '# owned by somebody else' "$output_file"

if "$repo_dir/packaging/host/linux/bin/plank-display-prepare" --cleanup \
    --config "$config_file" --output "$output_file" >/dev/null 2>&1; then
  echo "uninstall cleanup removed a foreign Xorg configuration" >&2
  exit 1
fi
printf '%s\n' '# Generated by PLANK; do not edit.' >"$output_file"
"$repo_dir/packaging/host/linux/bin/plank-display-prepare" --cleanup \
  --config "$config_file" --output "$output_file" >/dev/null
[[ ! -e $output_file ]]


# --- hybrid ------------------------------------------------------------------
rm -f -- "$output_file"
inventory_file="${work_dir}/display-inventory"
xorg_conf="${work_dir}/xorg.conf"
pci_dir="${work_dir}/pci-nvidia"
version_file="${work_dir}/nvidia-version"
mkdir -p "${pci_dir}/0000:01:00.0" "${pci_dir}/module"
printf '580.159.04\n' >"$version_file"
# A DKU-style baseline: an unused dual-GPU device and screen, and the active
# screen's device with Overlay (depth 24).
cat >"$xorg_conf" <<'EOF'
Section "ServerLayout"
    Screen          0  "Screen Default 0"     0 0
    Identifier     "Default Layout"
EndSection

Section "Device"
    Identifier  "Dual GPU"
    Driver      "nvidia"
    Option      "UseDisplayDevice" "none"
    Option      "HardDPMS" "False"
EndSection

Section "Screen"
    Identifier "Dual GPU Screen"
    Device     "Dual GPU"
    DefaultDepth 30
EndSection

Section "Device"
    Identifier  "NVIDIA Generic"
    Driver      "nvidia"
    Option      "Overlay" "on"
    Option      "HardDPMS" "False"
    Option      "Interactive" "False"
    Option      "UseDisplayDevice" "DFP-9"
#    Option      "MetaModes" "DFP-X: X1RESxY1RES +X1OFFSET+0"
EndSection

Section "Screen"
    Identifier     "Screen Default 0"
    Device         "NVIDIA Generic"
    DefaultDepth   24
    SubSection     "Display"
        Depth 24
    EndSubSection
EndSection
EOF

fingerprint=$(printf '%064d' 0 | tr 0 a)
write_inventory() {
  local physical=${1:-dongle}
  {
    echo '# Generated by PLANK; do not edit.'
    echo 'version=1'
    echo "fingerprint=${fingerprint}"
    echo 'gpu_bus_id=PCI:1:0:0'
    echo 'driver_version=580.159.04'
    echo 'startup_policy=hybrid'
    echo 'screen_virtual='
    if [[ $physical == two ]]; then
      echo 'physical_count=2'
      echo 'physical_1_dfp=DFP-5'
      echo 'physical_1_dpy=DPY-5'
      echo 'physical_1_output=DP-4'
      echo 'physical_1_connector=Connector-0'
      echo 'physical_1_display_name=Built-in display'
      echo 'physical_1_edid_sha256='
      echo 'physical_1_preferred=1920x1080'
      echo 'physical_1_modes=1920x1080'
      echo 'physical_2_dfp=DFP-4'
      echo 'physical_2_dpy=DPY-4'
      echo 'physical_2_output=HDMI-0'
      echo 'physical_2_connector=Connector-3'
      echo 'physical_2_display_name=MEC-O-3-H'
      echo 'physical_2_edid_sha256=0d7da54a596598e1b3611d1c138880021ff7ca1e827309a7658dbe7b4b6b1dd5'
      echo 'physical_2_preferred=3840x2160'
      echo 'physical_2_modes=4096x2160,3840x2160,1920x1080'
    else
      echo 'physical_count=1'
      echo 'physical_1_dfp=DFP-4'
      echo 'physical_1_dpy=DPY-4'
      echo 'physical_1_output=HDMI-0'
      echo 'physical_1_connector=Connector-3'
      echo "physical_1_display_name=${display_name_value:-MEC-O-3-H}"
      echo 'physical_1_edid_sha256=0d7da54a596598e1b3611d1c138880021ff7ca1e827309a7658dbe7b4b6b1dd5'
      echo 'physical_1_preferred=3840x2160'
      echo 'physical_1_modes=4096x2160,3840x2160,1920x1080,1280x1024,1280x720,1024x768,720x480,640x480'
    fi
    echo 'virtual_candidate_count=3'
    echo 'virtual_candidate_1_dfp=DFP-0'
    echo 'virtual_candidate_1_dpy=DPY-0'
    echo 'virtual_candidate_1_output=DP-0'
    echo 'virtual_candidate_1_connector=Connector-1'
    echo 'virtual_candidate_2_dfp=DFP-2'
    echo 'virtual_candidate_2_dpy=DPY-2'
    echo 'virtual_candidate_2_output=DP-2'
    echo 'virtual_candidate_2_connector=Connector-2'
    echo 'virtual_candidate_3_dfp=DFP-6'
    echo 'virtual_candidate_3_dpy=DPY-6'
    echo 'virtual_candidate_3_output=DP-5'
    echo 'virtual_candidate_3_connector=Connector-4'
    echo 'virtual_heads=0'
  } >"$inventory_file"
  chmod 0600 "$inventory_file"
}

run_hybrid_prepare() {
  PATH="${fake_bin}:${PATH}" \
    "$repo_dir/packaging/host/linux/bin/plank-display-prepare" \
      --config "$config_file" --output "$output_file" --edid-dir "$edid_dir" \
      --inventory "$inventory_file" --xorg-conf "$xorg_conf" \
      --pci-driver-dir "$pci_dir" --nvidia-version-file "$version_file" "$@"
}

expect_physical_fallback() {
  local description=$1
  shift
  run_hybrid_prepare "$@" >/dev/null 2>&1 || {
    echo "hybrid preparation failed instead of falling back: ${description}" >&2
    exit 1
  }
  if [[ -e $output_file ]]; then
    echo "hybrid preparation wrote an overlay despite: ${description}" >&2
    exit 1
  fi
}

printf '[display]\nstartup_layout = hybrid\nadapter_name =\n' >"$config_file"

# Without a cache the host starts exactly like `physical`.
rm -f -- "$inventory_file"
expect_physical_fallback "a missing display inventory"

write_inventory
run_hybrid_prepare >/dev/null
grep -Fxq "# Inventory fingerprint: ${fingerprint}" "$output_file"
grep -Fq 'BusID "PCI:1:0:0"' "$output_file"
grep -Fq 'Option "ConnectedMonitor" "DFP-4, DFP-0, DFP-2, DFP-6"' "$output_file"
grep -Fq 'Option "UseDisplayDevice" "DFP-4, DFP-0, DFP-2, DFP-6"' "$output_file"
grep -Fq "Option \"CustomEDID\" \"DFP-0:${edid_dir}/virtual-1.edid; DFP-2:${edid_dir}/virtual-2.edid; DFP-6:${edid_dir}/virtual-3.edid\"" "$output_file"
grep -Fq 'Option "MetaModes" "DFP-4: nvidia-auto-select +0+0, DFP-0: NULL, DFP-2: NULL, DFP-6: NULL"' "$output_file"
grep -Fq 'Virtual 16384 4608' "$output_file"
# The DKU device options and depth are carried; its UseDisplayDevice is not.
grep -Fxq '    Option "Overlay" "on"' "$output_file"
grep -Fxq '    Option "HardDPMS" "False"' "$output_file"
grep -Fxq '    Option "Interactive" "False"' "$output_file"
grep -Fxq '    DefaultDepth 24' "$output_file"
grep -Fxq '        Depth 24' "$output_file"
if grep -Eq 'DFP-9|Depth 30|UseDisplayDevice" "none' "$output_file"; then
  echo "the hybrid overlay copied an inactive or PLANK-owned DKU option" >&2
  exit 1
fi
[[ $(grep -c 'Option "UseDisplayDevice"' "$output_file") -eq 1 ]]
if grep -Fq 'virtual-4.edid' "$output_file"; then
  echo "the hybrid overlay reserved a fourth virtual head" >&2
  exit 1
fi
run_hybrid_prepare | grep -Fq 'already current'

# Two physical outputs, left to right at their preferred widths.
write_inventory two
run_hybrid_prepare >/dev/null
grep -Fq 'Option "MetaModes" "DFP-5: nvidia-auto-select +0+0, DFP-4: nvidia-auto-select +1920+0, DFP-0: NULL, DFP-2: NULL, DFP-6: NULL"' "$output_file"
grep -Fq 'Option "ConnectedMonitor" "DFP-5, DFP-4, DFP-0, DFP-2, DFP-6"' "$output_file"

# Without a DKU baseline the depth is 24 and nothing is carried.
missing_conf="${work_dir}/missing.conf"
PATH="${fake_bin}:${PATH}" \
  "$repo_dir/packaging/host/linux/bin/plank-display-prepare" \
    --config "$config_file" --output "$output_file" --edid-dir "$edid_dir" \
    --inventory "$inventory_file" --xorg-conf "$missing_conf" \
    --pci-driver-dir "$pci_dir" --nvidia-version-file "$version_file" >/dev/null
grep -Fxq '    DefaultDepth 24' "$output_file"
if grep -Fq 'Overlay' "$output_file"; then
  echo "the hybrid overlay invented a DKU option" >&2
  exit 1
fi

# A cache from another GPU, driver or with unsafe content falls back.
write_inventory
sed -i 's/^gpu_bus_id=PCI:1:0:0$/gpu_bus_id=PCI:2:0:0/' "$inventory_file"
expect_physical_fallback "a cache from another GPU"
write_inventory
printf '580.126.09\n' >"$version_file"
expect_physical_fallback "a cache from another driver"
printf '580.159.04\n' >"$version_file"
mkdir -p "${pci_dir}/0000:02:00.0"
write_inventory
expect_physical_fallback "two NVIDIA GPUs"
rmdir "${pci_dir}/0000:02:00.0"
write_inventory
chmod 0644 "$inventory_file"
expect_physical_fallback "a world-readable cache"
display_name_value='$(reboot)' write_inventory
expect_physical_fallback "a cache with shell syntax"
write_inventory
printf 'physical_1_dfp=DFP-7\n' >>"$inventory_file"
expect_physical_fallback "a duplicate cache key"
write_inventory
sed -i 's/^virtual_candidate_count=3$/virtual_candidate_count=0/' "$inventory_file"
expect_physical_fallback "no free DisplayPort head"

# Hybrid never takes the virtual policy's persistent transitions, and unknown
# [display] options are still rejected.
write_inventory
if run_hybrid_prepare --layout single --mode-1 3840x2160 >/dev/null 2>&1; then
  echo "a virtual transition bypassed the hybrid display policy" >&2
  exit 1
fi
printf '[display]\nstartup_layout = hybrid\nvirtual_heads = 3\n' >"$config_file"
if run_hybrid_prepare >/dev/null 2>&1; then
  echo "an unknown [display] option was accepted" >&2
  exit 1
fi
rm -f -- "$output_file"

echo "display_prepare_tests=pass"
