#!/usr/bin/env bash

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color
TAGFAIL=$RED'[FAIL]'$NC
TAGPASS=$GREEN'[PASS]'$NC

PROJECT_DIR=$(dirname $(cargo locate-project | awk -F\" '{print $4}'))
# Some of the examples assume that the working directory is the project root
# and it never hurts to force consistency regardless
cd $PROJECT_DIR

EXAMPLES_DIR=tests

if [ -z "$TOOLCHAIN" ]; then
    TOOLCHAIN=$(rustc --version | sed 's/rustc [0-9\.\-]*\(.*\) (.*)/\1/')
fi

# Create expected output for fn-root-vars
echo $HOME > $EXAMPLES_DIR/fn-root-vars.out # Overwrite previous file
echo '${x::1B}]0;${USER}: ${PWD}${x::07}${c::0x55,bold}${USER}${c::default}:${c::0x4B}${SWD}${c::default}# ${c::reset}' >> $EXAMPLES_DIR/fn-root-vars.out
id -u $USER >> $EXAMPLES_DIR/fn-root-vars.out
echo >> $EXAMPLES_DIR/fn-root-vars.out

test() {
    # Replace .ion with .out in file name
    EXPECTED_OUTPUT_FILE=$(echo $1 | sed 's/\..\+/\.out/')

    # Compare real and expected output
    if target/debug/ion "${@:2}" 2>&1 | diff - "$EXPECTED_OUTPUT_FILE"; then
        echo -e "Test ${1} ${TAGPASS}";
        return 0;
    else
        echo -e "Test ${1} ${TAGFAIL}";
        return 1;
    fi
}

test_generic() {
    ls -1 $EXAMPLES_DIR/*.ion | xargs -P $CPU_CORES -n 1 -I {} bash -c "test {} {} 1"
}

test_params() {
    ls -1 $EXAMPLES_DIR/*.params | xargs -P $CPU_CORES -n 1 -I {} bash -c "IFS=$'\\n'; test {} "'$(< {})'
}

set -e -u

export -f test
export TAGFAIL
export TAGPASS
export EXAMPLES_DIR

CPU_CORES=$(nproc --all)

# Build debug binary
cargo +$TOOLCHAIN build
test_generic
test_params

set +u

if [ -n "$FULL" ]; then
    # Build debug binary for testing structopt argument parsing
    cargo +$TOOLCHAIN build --features=advanced_arg_parsing
    test_params
fi
