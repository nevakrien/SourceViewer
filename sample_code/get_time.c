#include <stdio.h>

#ifdef _WIN32
#include <windows.h>

void get_current_time() {
    SYSTEMTIME st;
    GetSystemTime(&st);
    printf("Time: %02d:%02d:%02d.%03d\n", st.wHour, st.wMinute, st.wSecond, st.wMilliseconds);
}

#else
#include <time.h>

void get_current_time() {
    struct timespec ts;
    if (clock_gettime(CLOCK_MONOTONIC, &ts) == 0) {
        printf("%lld.%09ld\n", (long long)ts.tv_sec, ts.tv_nsec);
    } else {
        perror("clock_gettime failed");
    }
}

#endif

int main() {
    get_current_time();
    return 0;
}
