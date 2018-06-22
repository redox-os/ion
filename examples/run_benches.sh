#!/usr/bin/env bash

set -e -u -o pipefail

EXAMPLES_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_DIR=$(dirname $(cargo locate-project | awk -F\" '{print $4}'))

TIMEFORMAT='%U seconds'

# Some of the examples assume that the working directory is the project root
# and it never hurts to force consistency regardless
cd $PROJECT_DIR

function check_timing {

    # Run script and redirect error to /dev/null
    utime="$( time ( $PROJECT_DIR/target/release/ion $1 2> /dev/null) 2>&1 1>/dev/null )"
    echo $1 $utime
}

# Build release binary
#cargo build --release

set +e
# Iterate over every Ion script in examples directory
for i in $EXAMPLES_DIR/*.ion; do
    check_timing $i >> $EXAMPLES_DIR/temp.out;
done

cat $EXAMPLES_DIR/temp.out | column -t;

rm $EXAMPLES_DIR/temp.out;

exit 0
