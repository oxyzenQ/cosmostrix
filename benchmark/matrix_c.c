// SPDX-License-Identifier: GPL-3.0-only
// Minimal full-redraw Matrix rain in C.
// Writes every cell every frame via ANSI escape sequences to stdout.
// Usage: BENCH_COLS=120 BENCH_LINES=40 BENCH_FRAMES=100 ./matrix_c
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

int main() {
    int cols = getenv("BENCH_COLS") ? atoi(getenv("BENCH_COLS")) : 120;
    int lines = getenv("BENCH_LINES") ? atoi(getenv("BENCH_LINES")) : 40;
    int frames = getenv("BENCH_FRAMES") ? atoi(getenv("BENCH_FRAMES")) : 100;
    char *buf = malloc(cols * lines);
    int *heads = malloc(cols * sizeof(int));
    srand(time(NULL));
    for (int c = 0; c < cols; c++) { heads[c] = rand() % lines; }
    for (int i = 0; i < cols * lines; i++) buf[i] = ' ';

    struct timespec t0, t1;
    clock_gettime(CLOCK_MONOTONIC, &t0);
    long total_bytes = 0;

    for (int f = 0; f < frames; f++) {
        for (int c = 0; c < cols; c++) {
            heads[c] = (heads[c] + 1) % lines;
            buf[heads[c] * cols + c] = "01"[rand() % 2];
            if (heads[c] > 0) buf[(heads[c]-1) * cols + c] = ' ';
        }
        // Full redraw: cursor home + every cell
        int n = printf("\x1b[H");
        for (int r = 0; r < lines; r++) {
            for (int c = 0; c < cols; c++) {
                char ch = buf[r * cols + c];
                if (ch != ' ') n += printf("\x1b[32m%c", ch);
                else n += printf(" ");
            }
            n += printf("\n");
        }
        fflush(stdout);
        total_bytes += n;
    }

    clock_gettime(CLOCK_MONOTONIC, &t1);
    double elapsed = (t1.tv_sec - t0.tv_sec) + (t1.tv_nsec - t0.tv_nsec) / 1e9;
    fprintf(stderr, "C: frames=%d elapsed=%.3f fps=%.1f bytes=%ld\n", frames, elapsed, frames/elapsed, total_bytes);
    free(buf); free(heads);
    return 0;
}
