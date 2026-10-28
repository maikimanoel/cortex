/* Test-only Linux/glibc interposition. Loaded only into an isolated test child.
 * No buffers, keys or paths are logged. Injected errno is NOT a device failure.
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static _Thread_local unsigned calls;

static void *next(const char *name) {
    void *p = dlsym(RTLD_NEXT, name);
    if (!p) _exit(93);
    return p;
}

static int selected(const char *op, const char *path) {
    const char *target = getenv("CORTEX_FAULT_TARGET");
    const char *operation = getenv("CORTEX_FAULT_OP");
    const char *arm = getenv("CORTEX_FAULT_ARM");
    return target && operation && arm && !access(arm, F_OK)
        && !strcmp(target, path) && !strcmp(operation, op);
}

static int selected_fd(const char *op, int fd) {
    char proc[64], path[PATH_MAX + 1];
    snprintf(proc, sizeof(proc), "/proc/self/fd/%d", fd);
    ssize_t n = readlink(proc, path, PATH_MAX);
    if (n < 0) return 0;
    path[n] = 0;
    return selected(op, path);
}

static int fail(void) {
    const char *n = getenv("CORTEX_FAULT_NTH");
    unsigned nth = n ? (unsigned)strtoul(n, NULL, 10) : 0;
    ++calls;
    return nth && calls == nth;
}

static void trace(const char *op, int injected, size_t requested, ssize_t result) {
    int saved = errno;
    /* At most 100 records; no data buffers enter the format string/arguments. */
    if (calls <= 100)
        dprintf(STDERR_FILENO, "cortex-fault %s %u %d %zu %zd %d\n",
                op, calls, injected, requested, result, result < 0 ? saved : 0);
    errno = saved;
}

int fsync(int fd) {
    int (*real)(int) = next("fsync");
    if (!selected_fd("sync", fd)) return real(fd);
    int injected = fail();
    int result = injected ? (errno = EIO, -1) : real(fd);
    trace("sync", injected, 0, result);
    return result;
}

int fdatasync(int fd) {
    int (*real)(int) = next("fdatasync");
    if (!selected_fd("sync", fd)) return real(fd);
    int injected = fail();
    int result = injected ? (errno = EIO, -1) : real(fd);
    trace("sync", injected, 0, result);
    return result;
}

ssize_t write(int fd, const void *buf, size_t size) {
    ssize_t (*real)(int, const void *, size_t) = next("write");
    if (!selected_fd("write", fd)) return real(fd, buf, size);
    int injected = fail();
    ssize_t result = injected ? (errno = EIO, -1) : real(fd, buf, size);
    trace("write", injected, size, result);
    return result;
}

ssize_t pwrite(int fd, const void *buf, size_t size, off_t offset) {
    ssize_t (*real)(int, const void *, size_t, off_t) = next("pwrite");
    if (!selected_fd("write", fd)) return real(fd, buf, size, offset);
    int injected = fail();
    ssize_t result = injected ? (errno = EIO, -1) : real(fd, buf, size, offset);
    trace("write", injected, size, result);
    return result;
}

ssize_t pwrite64(int fd, const void *buf, size_t size, off64_t offset) {
    ssize_t (*real)(int, const void *, size_t, off64_t) = next("pwrite64");
    if (!selected_fd("write", fd)) return real(fd, buf, size, offset);
    int injected = fail();
    ssize_t result = injected ? (errno = EIO, -1) : real(fd, buf, size, offset);
    trace("write", injected, size, result);
    return result;
}

int unlink(const char *path) {
    int (*real)(const char *) = next("unlink");
    if (!selected("unlink", path)) return real(path);
    int injected = fail();
    int result = injected ? (errno = EIO, -1) : real(path);
    trace("unlink", injected, 0, result);
    return result;
}
