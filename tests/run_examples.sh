#!/usr/bin/env bash

set -e -u -o pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color
TAGFAIL=$RED'[FAIL]'$NC
TAGPASS=$GREEN'[PASS]'$NC

EXAMPLES_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_DIR=$(dirname $(cargo locate-project | awk -F\" '{print $4}'))

if [ -z "$TOOLCHAIN" ]; then
    TOOLCHAIN=$(rustc --version | sed 's/rustc [0-9\.\-]*\(.*\) (.*)/\1/')
fi

EXIT_VAL=0

# Some of the examples assume that the working directory is the project root
# and it never hurts to force consistency regardless
cd $PROJECT_DIR

# Create expected output for fn-root-vars
echo $HOME > $EXAMPLES_DIR/fn-root-vars.out # Overwrite previous file
echo '${x::1B}]0;${USER}: ${PWD}${x::07}${c::0x55,bold}${USER}${c::default}:${c::0x4B}${SWD}${c::default}# ${c::reset}' >> $EXAMPLES_DIR/fn-root-vars.out
echo $UID >> $EXAMPLES_DIR/fn-root-vars.out
echo >> $EXAMPLES_DIR/fn-root-vars.out

function test {
    # Replace .ion with .out in file name
    EXPECTED_OUTPUT_FILE=$(echo $1 | sed 's/\..\+/\.out/')

    # Compare real and expected output
    if diff <($PROJECT_DIR/target/debug/ion "${@:2}" 2>&1) "$EXPECTED_OUTPUT_FILE"; then
        echo -e "Test ${1} ${TAGPASS}";
        return 0;
    else
        echo -e "Test ${1} ${TAGFAIL}";
        return 1;
    fi
}

function test_cli {
    # Check number of parameters passed into the check function
    if [[ $# -ne 1 ]]; then
        echo -e "Illegal number of parameters.${TAGFAIL}";
        return 1;
    fi

    # Run script and redirect stdout into tmp file
    IFS=$'\n'; test $1 $(< $1)
}

function check_return_value {
    # Check number of parameters passed into the check function
    if [[ $# -ne 1 ]]; then
        echo -e "Illegal number of parameters.${TAGFAIL}";
        return 1;
    fi

    # Run script and redirect stdout into tmp file
    test $1 $1 1
}

# Build debug binary
cargo +$TOOLCHAIN build
set +e
# Iterate over every Ion script in examples directory
for i in $EXAMPLES_DIR/*.ion; do
    if ! check_return_value $i; then
        EXIT_VAL=1;
    fi
done

# Iterate over every parameter set
for i in $EXAMPLES_DIR/*.params; do
    if ! test_cli $i; then
        EXIT_VAL=1;
    fi
done

# Build debug binary for testing structopt argument parsing
cargo +$TOOLCHAIN build --features=advanced_arg_parsing
# Iterate over every parameter set
for i in $EXAMPLES_DIR/*.params; do
    if ! test_cli $i; then
        EXIT_VAL=1;
    fi
done

set -e
exit $EXIT_VAL
