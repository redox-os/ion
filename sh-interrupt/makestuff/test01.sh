#!/bin/sh

export TESTMAKE
if [ ! "$TESTMAKE" ] ; then
    TESTMAKE=make
fi

echo 'You should be able to kill this script with just one SIGINT'
echo 'Problematic shell/make - Kombinations will enter make a second time'
set -x
$TESTMAKE -s -f Makefile1
$TESTMAKE -s -f Makefile1
