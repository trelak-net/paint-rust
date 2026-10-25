/* Process-local Linux save fault injection; never preload into the application. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <stddef.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <linux/filter.h>
#include <linux/seccomp.h>

static int selected(int fd) {
    const char *root = getenv("PAINT_RUST_FAULT_ROOT");
    char link[64], path[4096];
    if (!root) return 0;
    snprintf(link, sizeof(link), "/proc/self/fd/%d", fd);
    ssize_t n = readlink(link, path, sizeof(path) - 1);
    if (n < 0) return 0;
    path[n] = 0;
    return strncmp(path, root, strlen(root)) == 0 && path[strlen(root)] == '/';
}
static int mode(const char *value) {
    const char *current = getenv("PAINT_RUST_FAULT_MODE");
    return current && strcmp(current, value) == 0;
}
static size_t written;
ssize_t write(int fd, const void *buf, size_t count) {
    ssize_t (*real_write)(int, const void *, size_t) = dlsym(RTLD_NEXT, "write");
    if (mode("write") && selected(fd)) {
        if (written >= 128) { errno = ENOSPC; return -1; }
        if (count > 128 - written) count = 128 - written;
        ssize_t n = real_write(fd, buf, count);
        if (n > 0) written += n;
        return n;
    }
    return real_write(fd, buf, count);
}
int fsync(int fd) {
    int (*real_fsync)(int) = dlsym(RTLD_NEXT, "fsync");
    if (mode("sync") && selected(fd)) { errno = EIO; return -1; }
    return real_fsync(fd);
}
/* rustix can bypass libc for rename, so reject just those syscalls in this child. */
__attribute__((constructor)) static void restrict_replace(void) {
    if (!mode("rename")) return;
    struct sock_filter filter[] = {
        BPF_STMT(BPF_LD | BPF_W | BPF_ABS, offsetof(struct seccomp_data, nr)),
#ifdef SYS_rename
        BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, SYS_rename, 0, 1),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | EACCES),
#endif
        BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, SYS_renameat, 0, 1),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | EACCES),
        BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, SYS_renameat2, 0, 1),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | EACCES),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW),
    };
    struct sock_fprog program = { .len = sizeof(filter) / sizeof(filter[0]), .filter = filter };
    if (prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) || prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, &program))
        _exit(125);
}
