#pragma once

#include <stdbool.h>
#include <stddef.h>

// The remote's only input report (docs/report-map.bin) is the Report ID byte
// followed by one bit per button, in RemapUsages order, padded to whole bytes.
#define REMAP_REPORT_ID    3
#define REMAP_BUTTONS      19
#define REMAP_PAYLOAD_SIZE ((REMAP_BUTTONS + 7) / 8)
#define REMAP_REPORT_SIZE  (1 + REMAP_PAYLOAD_SIZE)

// Map target of a button that sends nothing.
#define REMAP_DROP 0xFE

// Consumer usage of each report bit, in report order.
extern const unsigned short RemapUsages[REMAP_BUTTONS];

// A remapping of the remote's buttons onto the usages its report carries.
typedef struct {
    // The report bit that each button's bit moves to, or REMAP_DROP.
    unsigned char Target[REMAP_BUTTONS];
} REMAP_MAP;

// Bit index of a Consumer usage in the report, or -1 if the report has none.
int RemapUsageToBit(unsigned usage);

// Sets every button to send its own usage.
void RemapInit(REMAP_MAP *map);

#define REMAP_USAGE_NONE 0

// Maps the button that sends usage `from` to send usage `to` instead, or to
// send nothing when `to` is REMAP_USAGE_NONE. Returns true if both usages are
// in the report. Otherwise returns false and leaves the map unchanged.
bool RemapSet(REMAP_MAP *map, unsigned from, unsigned to);

// Checks whether a report is the remote's input report with at least one
// button down. The padding bits do not count.
bool RemapIsPress(const unsigned char *report, size_t len);

// Rewrites the buttons of the remote's input report in place. Leaves any other
// report alone.
void RemapReport(const REMAP_MAP *map, unsigned char *report, size_t len);
