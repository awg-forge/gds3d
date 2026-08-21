#!/usr/bin/env bash

set -euo pipefail

readonly package_name="sealantern-connect"
readonly script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly repo_dir="$(cd -- "${script_dir}/.." && pwd)"
readonly output_dir="${repo_dir}/target/release/bundle/archlinux"

require_command() {
    local command_name="$1"
    if ! command -v "${command_name}" >/dev/null 2>&1; then
        echo "required command not found: ${command_name}" >&2
        exit 1
    fi
}

detect_architecture() {
    case "$(uname -m)" in
        x86_64)
            package_arch="x86_64"
            deb_arch="amd64"
            ;;
        aarch64)
            package_arch="aarch64"
            deb_arch="arm64"
            ;;
        *)
            echo "unsupported architecture: $(uname -m)" >&2
            exit 1
            ;;
    esac
}

read_version() {
    local version_tag
    version_tag="$(node "${repo_dir}/scripts/version.ts" show)"
    if [[ ! "${version_tag}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
        echo "invalid project version: ${version_tag}" >&2
        exit 1
    fi
    version="${version_tag#v}"
    package_version="${version//-/_}"
}

find_deb() {
    local deb_dir="${repo_dir}/target/release/bundle/deb"
    local -a candidates=()
    mapfile -d '' candidates < <(
        find "${deb_dir}" -maxdepth 1 -type f -name "*_${version}_${deb_arch}.deb" -print0
    )
    if [[ "${#candidates[@]}" -ne 1 ]]; then
        echo "expected one ${version} ${deb_arch} deb bundle, found ${#candidates[@]}" >&2
        exit 1
    fi
    deb_path="${candidates[0]}"
}

find_package() {
    local expected_name="${package_name}-${package_version}-1-${package_arch}.pkg.tar.zst"
    package_path="${output_dir}/${expected_name}"
    if [[ ! -f "${package_path}" ]]; then
        echo "pacman package not found: ${package_path}" >&2
        exit 1
    fi
}

write_pkgbuild() {
    local pkgbuild_path="$1"
    local source_name="$2"
    local source_sha="$3"

    cat >"${pkgbuild_path}" <<PKGBUILD
pkgname='${package_name}'
pkgver='${package_version}'
pkgrel=1
pkgdesc='Lightweight Minecraft Java Edition P2P multiplayer client powered by sculk'
arch=('${package_arch}')
url='https://github.com/SeaLantern-Studio/SeaLantern-Connect'
license=('Apache-2.0')
depends=('glibc' 'gcc-libs' 'webkit2gtk-4.1' 'gtk3' 'libayatana-appindicator' 'librsvg')
options=('!strip')
source=('${source_name}')
noextract=('${source_name}')
sha256sums=('${source_sha}')

package() {
    local unpack_dir="\${srcdir}/deb-data"
    mkdir -p "\${unpack_dir}"
    bsdtar -xf "\${srcdir}/${source_name}" -C "\${unpack_dir}"

    local data_tar
    data_tar="\$(find "\${unpack_dir}" -maxdepth 1 -type f -name 'data.tar.*' -print -quit)"
    if [[ -z "\${data_tar}" ]]; then
        echo 'failed to locate data.tar.* in local deb' >&2
        return 1
    fi
    bsdtar -xf "\${data_tar}" -C "\${pkgdir}"
}
PKGBUILD
}

build_package() {
    local build_dir
    local source_name
    local source_sha

    for command_name in node pnpm makepkg bsdtar find install sha256sum awk mktemp; do
        require_command "${command_name}"
    done

    cd "${repo_dir}"
    pnpm tauri build --bundles deb
    find_deb

    mkdir -p "${output_dir}"
    build_dir="$(mktemp -d "${TMPDIR:-/tmp}/sealantern-arch.XXXXXX")"
    (
        trap 'rm -rf -- "${build_dir}"' EXIT
        source_name="${package_name}-${version}-${package_arch}.deb"
        install -m 644 "${deb_path}" "${build_dir}/${source_name}"
        source_sha="$(sha256sum "${build_dir}/${source_name}" | awk '{print $1}')"
        write_pkgbuild "${build_dir}/PKGBUILD" "${source_name}" "${source_sha}"

        PKGDEST="${output_dir}" makepkg \
            --dir "${build_dir}" \
            --cleanbuild \
            --clean \
            --force \
            --noconfirm
    )
    find_package
    echo "built pacman package: ${package_path}"
}

install_package() {
    require_command sudo
    require_command pacman
    find_package
    sudo pacman -U "${package_path}"
}

main() {
    if [[ ! -r /etc/arch-release ]]; then
        echo "this script requires Arch Linux or an Arch-based distribution" >&2
        exit 1
    fi
    if [[ "$#" -ne 1 ]]; then
        echo "usage: makearch.sh build | install" >&2
        exit 2
    fi

    detect_architecture
    require_command node
    read_version

    case "$1" in
        build) build_package ;;
        install) install_package ;;
        *)
            echo "usage: makearch.sh build | install" >&2
            exit 2
            ;;
    esac
}

main "$@"
