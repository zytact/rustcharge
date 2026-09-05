#!/usr/bin/env bash
# Create or update a fake sysfs battery tree that the `battery` crate accepts.
# Usage: fake-battery.sh <sysfs-dir> <Charging|Discharging|Full|Unknown> <percent 0-100>
# Writing to an existing tree while rustcharge runs changes what the next poll reads.
set -euo pipefail

root="${1:?sysfs dir}"; status="${2:?status}"; percent="${3:?percent}"
bat="$root/BAT0"
mkdir -p "$bat"

full=5000000
now=$(awk -v f="$full" -v p="$percent" 'BEGIN { printf "%d", f * p / 100 }')

w() { printf '%s\n' "$2" > "$bat/$1"; }
w type Battery
w present 1
w technology Li-ion
w manufacturer RustchargeVerify
w model_name FakeBattery
w serial_number VERIFY-0001
w voltage_now 11000000
w voltage_min_design 11000000
w charge_full_design "$full"
w charge_full "$full"
w charge_now "$now"
w capacity "$percent"
w cycle_count 1
w status "$status"

echo "fake battery: $status ${percent}% -> $bat"
