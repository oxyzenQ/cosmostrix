/*
 * Reference Matrix Rain — minimal C implementation for benchmark baseline.
 *
 * This is a deliberately simple Matrix rain renderer used as a baseline
 * competitor for the cosmostrix benchmark script. It is NOT meant to be
 * feature-complete — it exists so the benchmark script can produce a
 * real comparison even when no other Matrix rain renderer is installed.
 *
 * Build: gcc -O2 -o refmatrix refmatrix.c
 * Run:   ./refmatrix
 *
 * Copyright (C) 2026 rezky_nightky
 * SPDX-License-Identifier: GPL-3.0-only
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <sys/ioctl.h>

/* ANSI color: bright green */
#define GREEN "\x1b[32m"
#define BRIGHT "\x1b[1m"
#define RESET "\x1b[0m"

/* Character set: katakana + digits (subset) */
static const char CHARS[] = {
    '0','1','2','3','4','5','6','7','8','9',
    'A','B','C','D','E','F','X','Y','Z',
    0xa6,0xa7,0xa8,0xa9,0xaa,0xab,0xac,0xad,0xae,0xaf,
    0xb0,0xb1,0xb2,0xb3,0xb4,0xb5,0xb6,0xb7,0xb8,0xb9,
    0xba,0xbb,0xbc,0xbd,0xbe,0xbf,0xc0,0xc1,0xc2,0xc3,
    0xc4,0xc5,0xc6,0xc7,0xc8,0xc9,0xca,0xcb,0xcc,0xcd,
    0xce,0xcf,0xd0,0xd1,0xd2,0xd3,0xd4,0xd5,0xd6,0xd7,
    0xd8,0xd9,0xda,0xdb,0xdc,0xdd,0xde,0xdf
};
#define NCHARS (sizeof(CHARS))

int main(int argc, char **argv) {
    int cols = 80;
    int rows = 24;
    int duration_s = 5;

    if (argc >= 2) duration_s = atoi(argv[1]);
    if (argc >= 3) cols = atoi(argv[2]);
    if (argc >= 4) rows = atoi(argv[3]);

    /* Try to get terminal size */
    struct winsize ws;
    if (ioctl(STDOUT_FILENO, TIOCGWINSZ, &ws) == 0 && ws.ws_col > 0) {
        if (argc < 3) cols = ws.ws_col;
        if (argc < 4) rows = ws.ws_row;
    }

    /* Per-column state: current row position */
    int *pos = malloc(cols * sizeof(int));
    int *speed = malloc(cols * sizeof(int));
    if (!pos || !speed) return 1;

    srand(time(NULL));
    for (int c = 0; c < cols; c++) {
        pos[c] = rand() % rows;
        speed[c] = 1 + (rand() % 3);
    }

    /* Hide cursor, clear screen */
    printf("\x1b[?25l\x1b[2J\x1b[H");

    time_t start = time(NULL);
    time_t end = start + duration_s;

    while (time(NULL) < end) {
        /* Move cursor home (no full clear — overwrite) */
        printf("\x1b[H");

        for (int r = 0; r < rows; r++) {
            for (int c = 0; c < cols; c++) {
                if (r == pos[c]) {
                    /* Head: bright white */
                    printf(BRIGHT "%c" RESET, CHARS[rand() % NCHARS]);
                } else if (r >= pos[c] - 5 && r < pos[c]) {
                    /* Trail: green */
                    printf(GREEN "%c" RESET, CHARS[rand() % NCHARS]);
                } else {
                    /* Empty: space */
                    putchar(' ');
                }
            }
            if (r < rows - 1) putchar('\n');
        }
        fflush(stdout);

        /* Advance columns */
        for (int c = 0; c < cols; c++) {
            pos[c] += speed[c];
            if (pos[c] > rows + 5) {
                pos[c] = -5;
                speed[c] = 1 + (rand() % 3);
            }
        }

        usleep(33000); /* ~30 FPS */
    }

    /* Restore terminal */
    printf("\x1b[0m\x1b[?25h\x1b[2J\x1b[H");
    free(pos);
    free(speed);
    return 0;
}
