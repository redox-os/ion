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
ending=$([ "$EUID" -eq 0 ] && echo "#" || echo "$")
echo '${x::1B}]0;${USER}: ${PWD}${x::07}${c::0x55,bold}${USER}${c::default}:${c::0x4B}${SWD}${c::default}'$ending' ${c::reset}' >> $EXAMPLES_DIR/fn-root-vars.out
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
    ls -1 $@ | xargs -P $CPU_CORES -I {} bash -c "test {} {} 1"
}

test_params() {
    ls -1 $@ | xargs -P $CPU_CORES -I {} bash -c "IFS=$'\\n'; test {} "'$(< {})'
}

test_single() {
    # Remove "test." prefix given by make rule: test.<some_test>
    local stripped="$1"
    local stripped="${stripped/test./}"
   
    # Construct path for an ion and params file with base name equaling test name
    local base_path_for_test="${EXAMPLES_DIR}/${stripped}"
    local ion_path="${base_path_for_test}.ion"
    local params_path="${base_path_for_test}.params"

    # if 1 then at least an ion or params file was found for a test
    local found_one_test=0
    
    # Check if file even exits otherwise file not found error is raised 
    # by test_generic and test_params
    if [ -e "$ion_path" ]; then 
        test_generic "$ion_path"
        found_one_test=1
    fi
    if [ -e "$params_path" ]; then 
        test_params "$params_path"
        found_one_test=1
    fi
    
    if [ "$found_one_test" -eq 0 ];then
        echo -e "${RED}No ion or params file found for test ${stripped}${NC}"
    fi
}

set -e -u

export -f test
export TAGFAIL
export TAGPASS
export EXAMPLES_DIR

CPU_CORES=$(nproc --all)

# Build debug binary
# Check if the variable $RUSTUP is set. If yes, the $TOOLCHAIN can be choosen via cargo, otherwise cargo might not now
# about that feature.
if [ -v RUSTUP ] && [ "${RUSTUP}" -eq 1 ]; then
	cargo +$TOOLCHAIN build
else
	cargo build
fi

if [ $# -eq 1 ]; then
    test_single "$@"
else 
    test_generic "$EXAMPLES_DIR/*.ion"
    test_params "$EXAMPLES_DIR/*.params"
fi

set +u
