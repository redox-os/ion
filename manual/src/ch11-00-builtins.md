# Builtin Commands
## random
###Synopsis
```
random
random SEED
random START END
random START STEP END
random choice [ITEMS...]
```

###Description

RANDOM generates a pseudo-random integer from a uniform distribution. The range (inclusive) is dependent on the arguments passed. No arguments indicate a range of [0; 32767]. If one argument is specified, the internal engine will be seeded with the argument for future invocations of RANDOM and no output will be produced. Two arguments indicate a range of [START; END]. Three arguments indicate a range of [START; END] with a spacing of STEP between possible outputs. RANDOM choice will select one random item from the succeeding arguments.

NOTE: Due to limitations int the rand crate, seeding is not yet implemented
