#include <stdio.h>
#include <unistd.h>
#include <signal.h>

int main(void)
{
  int i;
  signal(SIGINT, SIG_IGN);
  signal(SIGQUIT, SIG_IGN);
  for (i=20 ; i>0; i--) {
    sleep(1);
  }
  return 0;
}
