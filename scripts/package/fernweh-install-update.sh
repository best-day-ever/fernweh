#!/bin/bash
# Bundled with the signed app. The client verifies the broker pin and DMG
# SHA-256 before launching this process; we independently verify Apple signing.
set -euo pipefail

bundle_id() { /usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$1/Contents/Info.plist"; }
bundle_version() { /usr/libexec/PlistBuddy -c 'Print :PLANKVersion' "$1/Contents/Info.plist"; }
team_id() { /usr/bin/codesign -dv --verbose=2 "$1" 2>&1 | /usr/bin/sed -n 's/^TeamIdentifier=//p'; }

verify_app() {
    local candidate=$1 current=$2 version=$3
    [[ -d $candidate && -d $current ]]
    /usr/bin/codesign --verify --deep --strict "$candidate"
    [[ $(bundle_id "$candidate") == $(bundle_id "$current") ]]
    [[ $(bundle_version "$candidate") == "$version" ]]
    local expected_team actual_team
    expected_team=$(team_id "$current")
    actual_team=$(team_id "$candidate")
    [[ -n $expected_team && $actual_team == "$expected_team" ]]
}

replace_app() {
    local source=$1 current=$2 version=$3
    local parent target staged backup
    parent=$(dirname "$current")
    target="$parent/BDE fernweh.app"
    staged="$parent/.bde-fernweh-update-$$.app"
    backup="$parent/.bde-fernweh-backup-$$.app"
    [[ ${current##*/} == 'BDE Fernweh Client.app' || ${current##*/} == 'BDE fernweh.app' ]]
    [[ ! -e $target || $current == "$target" ]]
    verify_app "$source" "$current" "$version"
    /usr/bin/ditto "$source" "$staged"
    verify_app "$staged" "$current" "$version"
    if ! /bin/mv "$current" "$backup"; then
        /bin/rm -rf "$staged"
        return 1
    fi
    if ! /bin/mv "$staged" "$target"; then
        /bin/mv "$backup" "$current"
        /bin/rm -rf "$staged"
        return 1
    fi
    /bin/rm -rf "$backup"
}

if [[ ${1:-} == '--replace' ]]; then
    [[ $# == 4 ]]
    replace_app "$2" "$3" "$4"
    exit
fi

[[ $# == 4 ]]
dmg=$1
current=$2
old_pid=$3
version=$4
[[ -f $dmg && -d $current && $old_pid =~ ^[0-9]+$ && $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]
[[ $(basename "$(dirname "$dmg")") == bde-fernweh-update-* ]]

mount_dir=$(/usr/bin/mktemp -d /tmp/bde-fernweh-mount.XXXXXX)
cleanup() {
    /usr/bin/hdiutil detach "$mount_dir" -quiet >/dev/null 2>&1 || true
    /bin/rmdir "$mount_dir" >/dev/null 2>&1 || true
    /bin/rm -rf "$(dirname "$dmg")"
}
trap cleanup EXIT

fail() {
    /usr/bin/osascript -e 'display alert "BDE fernweh update failed" message "Check BDE fernweh in Applications or contact your studio administrator."' >/dev/null 2>&1 || true
    exit 1
}
trap fail ERR

    /usr/sbin/spctl --assess --type open --context context:primary-signature "$dmg"
    /usr/bin/hdiutil attach "$dmg" -quiet -nobrowse -readonly -mountpoint "$mount_dir"
    source_app="$mount_dir/BDE fernweh.app"
    verify_app "$source_app" "$current" "$version"
    for _ in {1..90}; do
        /bin/kill -0 "$old_pid" 2>/dev/null || break
        /bin/sleep 1
    done
    if /bin/kill -0 "$old_pid" 2>/dev/null; then
        false
    fi

    if [[ -w $(dirname "$current") && -w $current ]]; then
        replace_app "$source_app" "$current" "$version"
    else
        /usr/bin/osascript \
            -e 'on run argv' \
            -e 'set cmd to "/bin/bash " & quoted form of item 1 of argv & " --replace " & quoted form of item 2 of argv & " " & quoted form of item 3 of argv & " " & quoted form of item 4 of argv' \
            -e 'do shell script cmd with administrator privileges' \
            -e 'end run' \
            "$0" "$source_app" "$current" "$version"
    fi
    /usr/bin/open "$(dirname "$current")/BDE fernweh.app"
