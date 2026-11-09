#!/usr/bin/env bash
# qmake updates compiler defines, but make does not invalidate existing objects
# when PLANK_VERSION changes. Keep the two version consumers in sync with it.
plank_macos_client_prepare_version() {
    local build=$1 version=$2
    local stamp="$build/.plank-client-compiled-version"
    if [[ ! -f $stamp || $(cat "$stamp") != "$version" ]]; then
        # Clear the stamp first: a failed build may leave a mixture of versions,
        # including when the next attempt returns to the previous version.
        rm -f "$stamp"
        rm -f "$build/app/release/main.o" "$build/app/release/systemproperties.o"
    fi
}

plank_macos_client_record_version() {
    local build=$1 version=$2
    # Call only after make succeeds. An interrupted write fails closed on retry.
    printf '%s\n' "$version" > "$build/.plank-client-compiled-version"
}
