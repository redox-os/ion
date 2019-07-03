#! ./testshell

(trap 'echo SIGINT happend ; exit 1' 2; ./hardguy ; echo -n)
echo survived
(trap 'echo SIGINT happend ; exit 1' 2; ./hardguy)
echo survived

