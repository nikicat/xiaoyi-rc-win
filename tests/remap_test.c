// Unit test for the report remap.

#include <stdio.h>
#include <string.h>

#include "../driver/remap.h"

static int failures;

static void ExpectReport(const char *name, const unsigned char *got, const unsigned char *want)
{
    if (memcmp(got, want, REMAP_REPORT_SIZE) != 0) {
        printf("FAIL %s: got %02X %02X %02X %02X, want %02X %02X %02X %02X\n", name,
               got[0], got[1], got[2], got[3], want[0], want[1], want[2], want[3]);
        failures++;
    } else {
        printf("ok   %s\n", name);
    }
}

static void ExpectInt(const char *name, int got, int want)
{
    if (got != want) {
        printf("FAIL %s: got %d, want %d\n", name, got, want);
        failures++;
    } else {
        printf("ok   %s\n", name);
    }
}

int main(void)
{
    unsigned char map[REMAP_BUTTONS];

    ExpectInt("vol+ is bit 6", RemapUsageToBit(0xE9), 6);
    ExpectInt("bookmarks is bit 18", RemapUsageToBit(0x22A), 18);
    ExpectInt("unknown usage", RemapUsageToBit(0x30), -1);

    RemapInit(map);
    {
        unsigned char r[] = {3, 0x40, 0, 0}, want[] = {3, 0x40, 0, 0};
        RemapReport(map, r, sizeof(r));
        ExpectReport("identity map leaves vol+ alone", r, want);
    }

    ExpectInt("set vol+ -> play/pause", RemapSet(map, 0xE9, 0xCD), 1);
    {
        unsigned char r[] = {3, 0x40, 0, 0}, want[] = {3, 0x10, 0, 0};
        RemapReport(map, r, sizeof(r));
        ExpectReport("vol+ press becomes play/pause", r, want);
    }
    {
        unsigned char r[] = {3, 0, 0, 0}, want[] = {3, 0, 0, 0};
        RemapReport(map, r, sizeof(r));
        ExpectReport("release stays release", r, want);
    }
    {
        unsigned char r[] = {3, 0xC0, 0, 0}, want[] = {3, 0x90, 0, 0};
        RemapReport(map, r, sizeof(r));
        ExpectReport("both buttons: play/pause + vol-", r, want);
    }

    ExpectInt("set vol- -> bookmarks", RemapSet(map, 0xEA, 0x22A), 1);
    {
        unsigned char r[] = {3, 0x80, 0, 0}, want[] = {3, 0, 0, 0x04};
        RemapReport(map, r, sizeof(r));
        ExpectReport("cross-byte target", r, want);
    }

    ExpectInt("drop vol-", RemapSet(map, 0xEA, 0), 1);
    {
        unsigned char r[] = {3, 0x80, 0, 0}, want[] = {3, 0, 0, 0};
        RemapReport(map, r, sizeof(r));
        ExpectReport("dropped button sends nothing", r, want);
    }

    ExpectInt("reject unknown target", RemapSet(map, 0xE9, 0x30), 0);
    ExpectInt("map kept after reject", map[6], 4);
    {
        unsigned char r[] = {1, 0x40, 0, 0}, want[] = {1, 0x40, 0, 0};
        RemapReport(map, r, sizeof(r));
        ExpectReport("other report IDs untouched", r, want);
    }
    {
        unsigned char r[] = {3, 0x40, 0, 0}, want[] = {3, 0x40, 0, 0};
        RemapReport(map, r, 2);
        ExpectReport("short report untouched", r, want);
    }

    return failures ? 1 : 0;
}
