// Unit test for TrimReportDescriptor. Usage: trim_test <docs/report-map.bin>

#include <stdio.h>

#include "../driver/trim.h"
#include "expect.h"

static void ExpectSize(const char *name, size_t got, size_t want)
{
    if (!Expect(name, got == want)) {
        printf("     got %zu, want %zu\n", got, want);
    }
}

int main(int argc, char **argv)
{
    unsigned char map[512];
    size_t len;
    FILE *f;

    if (argc != 2 || fopen_s(&f, argv[1], "rb") != 0) {
        fprintf(stderr, "usage: trim_test <report-map.bin>\n");
        return 2;
    }
    len = fread(map, 1, sizeof(map), f);
    fclose(f);

    ExpectSize("real report map is 122 bytes", len, 122);
    ExpectSize("real report map trims to 75", TrimReportDescriptor(map, len), 75);
    ExpectSize("already trimmed map is unchanged", TrimReportDescriptor(map, 75), 75);

    {
        unsigned char zeros[16] = {0};
        ExpectSize("all zeros are left alone", TrimReportDescriptor(zeros, sizeof(zeros)), 16);
    }
    {
        unsigned char noEnd[] = {0x05, 0x0c, 0x09, 0x01, 0x00, 0x00};
        ExpectSize("padding without End Collection is left alone",
                   TrimReportDescriptor(noEnd, sizeof(noEnd)), sizeof(noEnd));
    }
    ExpectSize("empty input", TrimReportDescriptor(map, 0), 0);

    return ExpectExitCode();
}
