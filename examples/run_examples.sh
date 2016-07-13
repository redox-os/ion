#!/bin/bash

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color
TAGFAIL=$RED'[FAIL]'$NC
TAGPASS=$GREEN'[PASS]'$NC

EXAMPLES_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

EXIT_VAL=0

function check_return_value {

    # Check number of parameters passed into the check function
    if [[ $# -ne 1 ]]; then 
        echo -e "Illegal number of parameters.${TAGFAIL}";
        return 1;
    fi

    # Replace .ion with .out in file name
    EXPECTED_OUTPUT_FILE=$(echo $1 | sed 's/\.ion/\.out/')

    # Run script and redirect stdout into tmp file
    cargo run $1 1> $EXAMPLES_DIR/tmp.out 2> /dev/null
    # Compare real and expected output
    cmp --silent $EXAMPLES_DIR/tmp.out $EXPECTED_OUTPUT_FILE
    local RET=$?

    # Clean up the mess
    rm -f $EXAMPLES_DIR/tmp.out

    # Write result
    if [[ $RET -ne 0 ]]; then
        echo -e "Test ${1} ${TAGFAIL}";
        return 1;
    else
        echo -e "Test ${1} ${TAGPASS}";
        return 0;
    fi
}

# Iterate over every Ion script in examples directory
for i in $EXAMPLES_DIR/*.ion; do
    check_return_value $i;
    if [[ $? -ne 0 ]]; then
        EXIT_VAL=1;
    fi
done

exit $EXIT_VAL
