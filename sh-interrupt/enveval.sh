#! /bin/sh
unset FOO 
FOO=foocorrect eval echo \$FOO

FOO=wrong
BAR=barcorrect
FOO=foocorrect eval echo \$FOO \$BAR

FOO=foocorrect sh -c 'echo "$FOO $BAR"'

