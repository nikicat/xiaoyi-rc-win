#pragma once

// The remote's only input report (docs/report-map.bin): Report ID 3, then
// 19 one-bit Consumer usages and 5 bits of padding.
#define REMAP_REPORT_ID   3
#define REMAP_REPORT_SIZE 4     // including the Report ID byte
#define REMAP_BUTTONS     19
#define REMAP_KEEP        0xFF  // map entry: send the usage unchanged
#define REMAP_DROP        0xFE  // map entry: send nothing

// Consumer usage of each report bit, in report order.
extern const unsigned short RemapUsages[REMAP_BUTTONS];

// Bit index of a Consumer usage in the report, or -1 if the report has none.
int RemapUsageToBit(unsigned usage);

// Start with every button unchanged.
void RemapInit(unsigned char map[REMAP_BUTTONS]);

// Map the button with usage `from` to usage `to` (0 drops it). Returns 0 if
// either usage is not in the report, and leaves the map unchanged then.
int RemapSet(unsigned char map[REMAP_BUTTONS], unsigned from, unsigned to);

// Rewrite one report in place. Reports that aren't this remote's input
// report are left alone.
void RemapReport(const unsigned char map[REMAP_BUTTONS], unsigned char *report, unsigned long len);
