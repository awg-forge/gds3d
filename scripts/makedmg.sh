#!/usr/bin/env bash

set -euo pipefail

run_hdiutil() {
    hdiutil "$@" 2> >(sed '/^hdiutil: WARNING: .*deprecated/d' >&2)
}

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "DMG layout can only be configured on macOS" >&2
    exit 1
fi

if [[ "$#" -gt 1 ]]; then
    echo "usage: makedmg.sh [path-to.dmg]" >&2
    exit 2
fi

dmg_path="${1:-}"
if [[ -z "${dmg_path}" ]]; then
    while IFS= read -r -d '' candidate; do
        if [[ -z "${dmg_path}" || "${candidate}" -nt "${dmg_path}" ]]; then
            dmg_path="${candidate}"
        fi
    done < <(find target -type f -path '*/bundle/dmg/*.dmg' -print0)
fi

if [[ -z "${dmg_path}" || ! -f "${dmg_path}" ]]; then
    echo "DMG bundle not found" >&2
    exit 1
fi

dmg_path="$(cd "$(dirname "${dmg_path}")" && pwd)/$(basename "${dmg_path}")"
work_dir="$(mktemp -d "${TMPDIR:-/tmp}/gds3d-dmg-layout.XXXXXX")"
shadow_path="${work_dir}/layout.shadow"
fixed_path="${work_dir}/fixed.dmg"
front_app_id="$(osascript -e \
    'tell application "System Events" to get bundle identifier of first application process whose frontmost is true' \
    2>/dev/null || true)"
device=""
mount_point=""

cleanup() {
    if [[ -n "${mount_point}" ]]; then
        run_hdiutil detach "${mount_point}" >/dev/null 2>&1 || true
    fi
    rm -rf "${work_dir}"
}
trap cleanup EXIT

attach_output="$(run_hdiutil attach -nobrowse -shadow "${shadow_path}" "${dmg_path}")"
device="$(awk -F '\t' '/^\/dev\// { print $1; exit }' <<<"${attach_output}")"
mount_point="$(awk -F '\t' '/^\/dev\// && $3 != "" { mount = $3 } END { print mount }' \
    <<<"${attach_output}")"

if [[ -z "${device}" || -z "${mount_point}" || ! -d "${mount_point}" ]]; then
    echo "failed to mount DMG with a writable shadow" >&2
    exit 1
fi

shopt -s nullglob
app_paths=("${mount_point}"/*.app)
shopt -u nullglob
if [[ "${#app_paths[@]}" -ne 1 ]]; then
    echo "expected exactly one app bundle in ${mount_point}" >&2
    exit 1
fi
if [[ ! -f "${mount_point}/.background/dmg-background.png" ]]; then
    echo "DMG background not found" >&2
    exit 1
fi
if [[ ! -L "${mount_point}/Applications" ]]; then
    echo "Applications link not found" >&2
    exit 1
fi

volume_name="$(basename "${mount_point}")"
app_name="$(basename "${app_paths[0]}")"

osascript - "${volume_name}" "${app_name}" <<'APPLESCRIPT'
on run argv
    set volumeName to item 1 of argv
    set appName to item 2 of argv

    tell application "Finder"
        tell disk volumeName
            open
            tell container window
                set current view to icon view
                set toolbar visible to false
                set statusbar visible to false
                set bounds to {10, 60, 670, 460}
            end tell

            set opts to icon view options of container window
            tell opts
                set icon size to 128
                set text size to 16
                set arrangement to not arranged
            end tell

            set background picture of opts to file ".background:dmg-background.png"
            set position of item appName to {180, 198}
            set position of item "Applications" to {480, 198}
            set position of item ".background" to {770, 100}
            set position of item ".VolumeIcon.icns" to {770, 100}
            set extension hidden of item appName to true

            close
            open
            delay 3
        end tell
    end tell
end run
APPLESCRIPT

for _ in {1..10}; do
    if [[ -f "${mount_point}/.DS_Store" ]]; then
        break
    fi
    sleep 1
done
if [[ ! -f "${mount_point}/.DS_Store" ]]; then
    echo "Finder did not write .DS_Store" >&2
    exit 1
fi

run_hdiutil detach "${mount_point}" >/dev/null
device=""
mount_point=""

if [[ -n "${front_app_id}" ]]; then
    open -b "${front_app_id}" >/dev/null 2>&1 || true
fi

run_hdiutil convert \
    -format UDZO \
    -imagekey zlib-level=9 \
    -shadow "${shadow_path}" \
    -o "${fixed_path}" \
    "${dmg_path}" >/dev/null

if [[ ! -f "${fixed_path}" ]]; then
    echo "failed to merge DMG shadow" >&2
    exit 1
fi

mv "${fixed_path}" "${dmg_path}"
echo "updated DMG layout: ${dmg_path}"
