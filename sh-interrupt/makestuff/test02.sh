#!/bin/sh

export TESTMAKE
if [ ! "$TESTMAKE" ] ; then
    TESTMAKE=make
fi

echo 'You should be able to kill this script with just one SIGINT'
echo 'Problematic shell/make - Kombinations will enter make a second time'
echo "On SIGINT, make should print 'Interrupt target runs'"
set -x
$TESTMAKE -s -f Makefile1a
$TESTMAKE -s -f Makefile1a
