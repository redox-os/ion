#!./testshell

echo 'You should be able to end the script with one SIGINT'
(while :; do wc /kernel > /dev/null ; done)
