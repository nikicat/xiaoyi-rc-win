#include <string.h>

#include "remap.h"

// Bits of the last payload byte that carry no button.
#define PADDING_MASK \
    ((unsigned char)(0xFF & ~((1u << (REMAP_BUTTONS - 8 * (REMAP_PAYLOAD_SIZE - 1))) - 1)))

const unsigned short RemapUsages[REMAP_BUTTONS] = {
    0x0B5, 0x0B6, 0x0B7, 0x0B8, 0x0CD, 0x0E2, 0x0E9, 0x0EA,   // next, prev, stop, eject, play/pause, mute, vol+, vol-
    0x183, 0x18A, 0x192, 0x194,                               // AL media config, mail, calculator, local browser
    0x221, 0x223, 0x224, 0x225,                               // AC search, home, back, forward
    0x226, 0x227, 0x22A,                                      // AC stop, refresh, bookmarks
};

static bool BitIsSet(const unsigned char *bits, int bit)
{
    return (bits[bit / 8] >> (bit % 8)) & 1;
}

static void SetBit(unsigned char *bits, int bit)
{
    bits[bit / 8] |= (unsigned char)(1 << (bit % 8));
}

static bool IsInputReport(const unsigned char *report, size_t len)
{
    return len >= REMAP_REPORT_SIZE && report[0] == REMAP_REPORT_ID;
}

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

void RemapInit(REMAP_MAP *map)
{
    int i;

    for (i = 0; i < REMAP_BUTTONS; i++) {
        map->Target[i] = (unsigned char)i;
    }
}

bool RemapSet(REMAP_MAP *map, unsigned from, unsigned to)
{
    int src = RemapUsageToBit(from);
    int dst = to == REMAP_USAGE_NONE ? REMAP_DROP : RemapUsageToBit(to);

    if (src < 0 || dst < 0) {
        return false;
    }
    map->Target[src] = (unsigned char)dst;
    return true;
}

bool RemapIsPress(const unsigned char *report, size_t len)
{
    int i;

    if (!IsInputReport(report, len)) {
        return false;
    }
    for (i = 0; i < REMAP_BUTTONS; i++) {
        if (BitIsSet(report + 1, i)) {
            return true;
        }
    }
    return false;
}

void RemapReport(const REMAP_MAP *map, unsigned char *report, size_t len)
{
    unsigned char *buttons = report + 1;
    unsigned char in[REMAP_PAYLOAD_SIZE];
    int i;

    if (!IsInputReport(report, len)) {
        return;
    }
    memcpy(in, buttons, REMAP_PAYLOAD_SIZE);
    memset(buttons, 0, REMAP_PAYLOAD_SIZE - 1);
    buttons[REMAP_PAYLOAD_SIZE - 1] &= PADDING_MASK;   // keep the padding bits as they were

    for (i = 0; i < REMAP_BUTTONS; i++) {
        if (BitIsSet(in, i) && map->Target[i] != REMAP_DROP) {
            SetBit(buttons, map->Target[i]);
        }
    }
}
