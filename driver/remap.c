#include "remap.h"

const unsigned short RemapUsages[REMAP_BUTTONS] = {
    0x0B5, 0x0B6, 0x0B7, 0x0B8, 0x0CD, 0x0E2, 0x0E9, 0x0EA,   // next, prev, stop, eject, play/pause, mute, vol+, vol-
    0x183, 0x18A, 0x192, 0x194,                               // AL media config, mail, calculator, local browser
    0x221, 0x223, 0x224, 0x225,                               // AC search, home, back, forward
    0x226, 0x227, 0x22A,                                      // AC stop, refresh, bookmarks
};

int RemapUsageToBit(unsigned usage)
{
    int i;

    for (i = 0; i < REMAP_BUTTONS; i++) {
        if (RemapUsages[i] == usage) {
            return i;
        }
    }
    return -1;
}

void RemapInit(unsigned char map[REMAP_BUTTONS])
{
    int i;

    for (i = 0; i < REMAP_BUTTONS; i++) {
        map[i] = REMAP_KEEP;
    }
}

int RemapSet(unsigned char map[REMAP_BUTTONS], unsigned from, unsigned to)
{
    int src = RemapUsageToBit(from);
    int dst = to == 0 ? REMAP_DROP : RemapUsageToBit(to);

    if (src < 0 || dst < 0) {
        return 0;
    }
    map[src] = (unsigned char)dst;
    return 1;
}

void RemapReport(const unsigned char map[REMAP_BUTTONS], unsigned char *report, unsigned long len)
{
    unsigned char in[3];
    int i, dst;

    if (len < REMAP_REPORT_SIZE || report[0] != REMAP_REPORT_ID) {
        return;
    }
    in[0] = report[1];
    in[1] = report[2];
    in[2] = report[3];
    report[1] = report[2] = 0;
    report[3] &= 0xF8;                  // keep the padding bits as they were

    for (i = 0; i < REMAP_BUTTONS; i++) {
        if (!(in[i / 8] & (1 << (i % 8)))) {
            continue;
        }
        dst = map[i] == REMAP_KEEP ? i : map[i];
        if (dst != REMAP_DROP) {
            report[1 + dst / 8] |= (unsigned char)(1 << (dst % 8));
        }
    }
}
