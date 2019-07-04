#! ./testshell

set -T
echo pid $$
trap : 2
echo "You should be able to end this script with two SIGINT"
echo
(echo pid $$ ; trap "echo exit pid $$ ; exit 1" 2 ; ./hardguy)
(echo pid $$ ; trap "echo exit pid $$ ; exit 1" 2 ; ./hardguy)
