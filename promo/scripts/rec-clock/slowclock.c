// slowclock.so (LD_PRELOAD): run one process's clocks at SLOWCLOCK_RATE (0 < rate <= 1) of real time.
//
// record-footage.sh --4k runs `nerv tv --record` under it. The recorder renders frame i no earlier than
// i/fps on the process clock, and the demo feed scripts its drama (the BLOCKED agent) on that clock too;
// at 3840x2160 the recorder is slower than real time, so on the real clock the drama would race ahead of
// the frames. On a clock slowed below the recorder's speed it paces itself instead, and every clip tells
// the same story as its 1080p twin (the takeover on the same frame). The recorder's own "max lag" line
// reports in this clock: near 0 means the clip is in step.
//
// clock_gettime: each clock runs at `rate` from its first reading (CPU-time clocks stay real).
// nanosleep / clock_nanosleep: durations and deadlines are read in the slowed clock and converted.
// Build: cc -O2 -shared -fPIC -o slowclock.so slowclock.c -ldl
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdint.h>
#include <stdlib.h>
#include <time.h>

#define NCLK 16
static int (*real_gettime)(clockid_t, struct timespec *);
static int (*real_nanosleep)(const struct timespec *, struct timespec *);
static int (*real_clock_nanosleep)(clockid_t, int, const struct timespec *, struct timespec *);
static double rate = 1.0;
static int64_t base[NCLK]; // first real reading per clock, in ns (0: not read yet)

static int64_t ns(const struct timespec *t) { return (int64_t)t->tv_sec * 1000000000LL + t->tv_nsec; }
static struct timespec ts(int64_t n) {
  struct timespec t = {n / 1000000000LL, n % 1000000000LL};
  return t;
}

__attribute__((constructor)) static void init(void) {
  real_gettime = dlsym(RTLD_NEXT, "clock_gettime");
  real_nanosleep = dlsym(RTLD_NEXT, "nanosleep");
  real_clock_nanosleep = dlsym(RTLD_NEXT, "clock_nanosleep");
  const char *r = getenv("SLOWCLOCK_RATE");
  double v = r ? atof(r) : 1.0;
  if (v > 0 && v <= 1) rate = v;
}

static int scaled(clockid_t c) {
  return rate != 1.0 && c >= 0 && c < NCLK && c != CLOCK_PROCESS_CPUTIME_ID && c != CLOCK_THREAD_CPUTIME_ID;
}

// the clock's first real reading (set once, by whichever thread reads it first)
static int64_t base_of(clockid_t c, int64_t real) {
  int64_t none = 0;
  __atomic_compare_exchange_n(&base[c], &none, real, 0, __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST);
  return __atomic_load_n(&base[c], __ATOMIC_SEQ_CST);
}

int clock_gettime(clockid_t c, struct timespec *t) {
  int r = real_gettime(c, t);
  if (r != 0 || !scaled(c)) return r;
  int64_t real = ns(t);
  int64_t b = base_of(c, real);
  *t = ts(b + (int64_t)((double)(real - b) * rate));
  return 0;
}

int nanosleep(const struct timespec *req, struct timespec *rem) {
  if (rate == 1.0) return real_nanosleep(req, rem);
  struct timespec real = ts((int64_t)((double)ns(req) / rate)), left = {0, 0};
  int r = real_nanosleep(&real, &left);
  if (rem) *rem = ts((int64_t)((double)ns(&left) * rate));
  return r;
}

int clock_nanosleep(clockid_t c, int flags, const struct timespec *req, struct timespec *rem) {
  if (!scaled(c)) return real_clock_nanosleep(c, flags, req, rem);
  if (flags & TIMER_ABSTIME) {
    struct timespec now;
    real_gettime(c, &now);
    int64_t b = base_of(c, ns(&now));
    struct timespec real = ts(b + (int64_t)((double)(ns(req) - b) / rate));
    return real_clock_nanosleep(c, flags, &real, NULL);
  }
  struct timespec real = ts((int64_t)((double)ns(req) / rate)), left = {0, 0};
  int r = real_clock_nanosleep(c, flags, &real, &left);
  if (rem) *rem = ts((int64_t)((double)ns(&left) * rate));
  return r;
}
