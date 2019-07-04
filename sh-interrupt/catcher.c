#undef VERBOSE

#include <stdio.h>
#include <signal.h>
#include <unistd.h>
#include <fcntl.h>
#include <errno.h>

#include <sys/types.h>
#include <termios.h>

#ifdef __FreeBSD__
#include <sys/ttydefaults.h>
#else
#define CTRL(x) (x&037)
#define  CEOF            CTRL('d')
#endif

#define BUFSIZE 65536

struct termios ttystate;
struct termios oldttystate;
int cleanupP = 0;

#ifndef CTRL
#define CTRL(x) (x&037)
#endif

int _global_fd;

void handler2(int sig);
void handler3(int sig);
void cleanup(void);

void handler2(int sig)
{
#define TMP "Async action on sigint (2)\n"
  write(1,TMP,sizeof(TMP)-1);
#undef TMP
}

void handler3(int sig)
{
#define TMP "Async action on sigquit (3)\n"
  write(1,TMP,sizeof(TMP)-1);
#undef TMP
}

void cleanup()
{
  if (cleanupP) {
    printf("Resettung terminal\n");
    if (tcsetattr(_global_fd, TCSANOW, &oldttystate) < 0) {
      perror("ioctl reset /dev/tty");
    }
  }
  close(_global_fd);
}

static void exit_handler(int sig)
#ifdef __GNUC__
    __attribute__ ((noreturn))
#endif
;
static void exit_handler(int sig)
{
  cleanup();
  if (sig)
    printf("Exiting on signal %d\n",sig);
  exit(0);
}

int main(void)
{
  char c[BUFSIZE];
  pid_t pgrp;

#ifdef VERBOSE
  printf("I'm PID %d\n",getpid());
#endif

  if ( (  _global_fd = open("/dev/tty",O_RDONLY)) < 1) {
    perror("open /dev/tty");
    exit_handler(0);
  }
  
  if ( (pgrp = tcgetpgrp(_global_fd)) < 0) {
    perror("Can't get pgrp\n");
    exit_handler(0);
  }
#ifdef VERBOSE
  printf("tty pgrp is %ld\n",(long)pgrp);
#endif

  if ( tcsetpgrp(_global_fd, pgrp) < 0) {
    perror("Can't set pgrp\n");
    exit_handler(0);
  }

  if (tcgetattr(_global_fd, &oldttystate) < 0) {
    perror("ioctl1 /dev/tty");
    exit_handler(0);
  }
  ttystate = oldttystate;
  ttystate.c_lflag &= ~ICANON;
  ttystate.c_lflag &= ~ECHO;
  ttystate.c_cc[VQUIT] = CTRL('g'); /* From sys/ttydefaults.h */
  if (tcsetattr(_global_fd, TCSANOW, &ttystate) < 0) {
    perror("ioctl2 /dev/tty");
    exit_handler(0);
  }
  cleanupP = 1;

  {
    struct sigaction siga;
    
    sigemptyset(&siga.sa_mask);
    siga.sa_flags = 0;

    siga.sa_handler = handler2;
    sigaction(SIGINT, &siga, (struct sigaction *)0);
    siga.sa_handler = handler3;
    sigaction(SIGQUIT, &siga, (struct sigaction *)0);

    siga.sa_handler = exit_handler;
    sigaction(SIGHUP, &siga, (struct sigaction *)0);
    sigaction(SIGTERM, &siga, (struct sigaction *)0);
  }

  printf("Use C-c and C-g for async actions, end with C-d\n");
  while (1) {
    switch (read(_global_fd,c,1)) {
    case -1:
      if (errno == EINTR)
	continue;
      perror("stdin read");
      exit_handler(0);
    case 0:
      printf("Exiting on stdin EOF (should happen only in cannon mode\n");
      exit_handler(0);
    default:
      if (c[0] == CEOF) { /* From sys/ttydefaults.h */
	printf("Exiting on stdin EOF (hopefully only in noncannon mode)\n");
	exit_handler(0);
      }
      printf("You typed: '%c' (0x%X)\n",c[0],c[0]);
    }
  }
  exit_handler(0);
  return 0;
}
