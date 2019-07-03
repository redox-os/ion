#! /bin/sh

test -d foo1 || mkdir foo1
test -d foo2 || mkdir foo2
test -d foo2 || mkdir foo3
echo 'echo :one' > foo1/run
echo 'echo :two' > foo2/run
echo 'echo :three' > foo2/run3
chmod a+x */run*

hash -r
PATH=./foo3:./foo1:./foo2:./foo5

echo Expect one:
PATH=./foo3:./foo3:./foo1 run
echo $PATH
echo ERROR: run should be in in foo1, but is in two in old sh:
hash -v
echo ERROR: should give one, but does two in old sh:
run

hash -r
echo
echo Expect two:
PATH=./foo3:./foo4:./foo3:./foo2:./foo5
run
hash -v
echo ERROR: Expect one, does not find run on old sh:
PATH=./foo3:./foo3:./foo1 run

echo
hash -r
PATH=./foo3:./foo1:./foo4:./foo5

echo Expect one, error preparation:
PATH=./foo3:./foo4:./foo1 run
echo Should show run in the wrong place:
hash -v
echo ERROR: Will not find run in old sh, should give one:
run

echo
echo expect one
PATH=./foo1:./foo2
run
echo expect three...
PATH=./foo3:./foo4:./foo2 run3
echo ERROR ... and now a coredump
hash -v
