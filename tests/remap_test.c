// Unit test for the report remap.

#include <stdio.h>
#include <string.h>

#include "../driver/remap.h"
#include "expect.h"

#define USAGE_POWER       0x030   // a Consumer usage the report does not carry
#define USAGE_PLAY_PAUSE  0x0CD
#define USAGE_VOLUME_UP   0x0E9
#define USAGE_VOLUME_DOWN 0x0EA
#define USAGE_BOOKMARKS   0x22A

// Every report bit past the buttons.
#define PADDING (~0ul << REMAP_BUTTONS)

// Returns the mask of a usage's bit in the report's buttons.
static unsigned long Button(unsigned usage)
{
    return 1ul << RemapUsageToBit(usage);
}

// Writes the remote's input report with the given bits set into a buffer.
static void MakeReport(unsigned char report[REMAP_REPORT_SIZE], unsigned long bits)
{
    int i;

    report[0] = REMAP_REPORT_ID;
    for (i = 0; i < REMAP_PAYLOAD_SIZE; i++) {
        report[1 + i] = (unsigned char)(bits >> (8 * i));
    }
}

static void PrintReport(const char *label, const unsigned char *report)
{
    int i;

    printf("     %s", label);
    for (i = 0; i < REMAP_REPORT_SIZE; i++) {
        printf(" %02X", report[i]);
    }
    printf("\n");
}

static void ExpectInt(const char *name, int got, int want)
{
    if (!Expect(name, got == want)) {
        printf("     got %d, want %d\n", got, want);
    }
}

static void ExpectReport(const char *name, const unsigned char *got, const unsigned char *want)
{
    if (!Expect(name, memcmp(got, want, REMAP_REPORT_SIZE) == 0)) {
        PrintReport("got ", got);
        PrintReport("want", want);
    }
}

// Checks that RemapReport turns the input report with the pressed bits set
// into the one with the wanted bits set.
static void ExpectRemap(const char *name, const REMAP_MAP *map, unsigned long pressed,
                        unsigned long want)
{
    unsigned char got[REMAP_REPORT_SIZE], expected[REMAP_REPORT_SIZE];

    MakeReport(got, pressed);
    MakeReport(expected, want);
    RemapReport(map, got, sizeof(got));
    ExpectReport(name, got, expected);
}

// Checks that RemapReport, given len bytes of the report, leaves it unchanged.
static void ExpectUntouched(const char *name, const REMAP_MAP *map, unsigned char *report,
                            size_t len)
{
    unsigned char want[REMAP_REPORT_SIZE];

    memcpy(want, report, REMAP_REPORT_SIZE);
    RemapReport(map, report, len);
    ExpectReport(name, report, want);
}

static void TestIsPress(void)
{
    unsigned char r[REMAP_REPORT_SIZE];

    MakeReport(r, 0);
    Expect("release is no press", !RemapIsPress(r, sizeof(r)));
    MakeReport(r, Button(USAGE_VOLUME_UP));
    Expect("vol+ is a press", RemapIsPress(r, sizeof(r)));
    MakeReport(r, Button(USAGE_BOOKMARKS));
    Expect("bookmarks in the last byte is a press", RemapIsPress(r, sizeof(r)));
    MakeReport(r, PADDING);
    Expect("padding alone is no press", !RemapIsPress(r, sizeof(r)));
    MakeReport(r, Button(USAGE_VOLUME_UP));
    Expect("short report is no press", !RemapIsPress(r, sizeof(r) - 1));
    r[0] = REMAP_REPORT_ID + 1;
    Expect("other report ID is no press", !RemapIsPress(r, sizeof(r)));
}

int main(void)
{
    REMAP_MAP map;
    unsigned long volumeUp = Button(USAGE_VOLUME_UP);
    unsigned long volumeDown = Button(USAGE_VOLUME_DOWN);
    unsigned long playPause = Button(USAGE_PLAY_PAUSE);

    ExpectInt("vol+ is bit 6", RemapUsageToBit(USAGE_VOLUME_UP), 6);
    ExpectInt("bookmarks is bit 18", RemapUsageToBit(USAGE_BOOKMARKS), 18);
    ExpectInt("unknown usage", RemapUsageToBit(USAGE_POWER), -1);

    TestIsPress();

    RemapInit(&map);
    ExpectRemap("identity map leaves vol+ alone", &map, volumeUp, volumeUp);

    Expect("set vol+ -> play/pause", RemapSet(&map, USAGE_VOLUME_UP, USAGE_PLAY_PAUSE));
    ExpectRemap("vol+ press becomes play/pause", &map, volumeUp, playPause);
    ExpectRemap("release stays release", &map, 0, 0);
    ExpectRemap("both buttons: play/pause + vol-", &map, volumeUp | volumeDown,
                playPause | volumeDown);
    ExpectRemap("padding bits are kept", &map, volumeUp | PADDING, playPause | PADDING);

    Expect("set vol- -> bookmarks", RemapSet(&map, USAGE_VOLUME_DOWN, USAGE_BOOKMARKS));
    ExpectRemap("cross-byte target", &map, volumeDown, Button(USAGE_BOOKMARKS));

    Expect("drop vol-", RemapSet(&map, USAGE_VOLUME_DOWN, REMAP_USAGE_NONE));
    ExpectRemap("dropped button sends nothing", &map, volumeDown, 0);

    Expect("reject unknown target", !RemapSet(&map, USAGE_VOLUME_UP, USAGE_POWER));
    ExpectRemap("map kept after reject", &map, volumeUp, playPause);
    {
        unsigned char r[REMAP_REPORT_SIZE];

        MakeReport(r, volumeUp);
        r[0] = REMAP_REPORT_ID + 1;
        ExpectUntouched("other report IDs untouched", &map, r, sizeof(r));
        MakeReport(r, volumeUp);
        ExpectUntouched("short report untouched", &map, r, sizeof(r) - 1);
    }

    return ExpectExitCode();
}
