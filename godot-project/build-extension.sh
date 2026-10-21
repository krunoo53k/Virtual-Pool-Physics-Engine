#!/bin/sh
# Builds the extension and puts it where the Godot project expects it.
#
#     ./godot-project/build-extension.sh          # the release build
#     ./godot-project/build-extension.sh debug    # the debug build
#
# The Godot project loads the library from `godot-project/bin/`, and the
# `.gdextension` file points at the files by name.

set -eu

profile=release
if [ "${1:-}" = "debug" ]; then
    profile=debug
fi

root=$(dirname "$0")/..
cd "$root"

echo "building the extension ($profile)"
cargo build --profile "$profile" -p pool-godot

case "$(uname -s)" in
    Darwin) extension=dylib ;;
    MINGW*|MSYS*|CYGWIN*) extension=dll ;;
    *) extension=so ;;
esac

if [ "$profile" = "release" ]; then
    built="target/release/libpool_godot.$extension"
else
    built="target/debug/libpool_godot.$extension"
fi

cp "$built" "godot-project/bin/"
echo "copied $built to godot-project/bin/"

# Godot reads the list of extensions to load from this file. The editor writes
# it when it opens the project, but a headless run needs it to be there.
mkdir -p godot-project/.godot
echo "res://bin/pool_godot.gdextension" > godot-project/.godot/extension_list.cfg
echo "wrote godot-project/.godot/extension_list.cfg"
