/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "plank_clipboard_file_wire.h"

#include <assert.h>
#include <stdio.h>
#include <string.h>

int main(void) {
    uint8_t offer[16] = {1};
    uint8_t record[PLANK_CLIPBOARD_FILE_HEADER_BYTES + 64] = {0};
    PlankClipboardFileRecord decoded;

    assert(plank_clipboard_file_header(record, sizeof(record),
        PLANK_CLIPBOARD_FILE_OFFER_BEGIN, 0, 0, offer, 7, 1));
    assert(plank_clipboard_file_decode(record, PLANK_CLIPBOARD_FILE_HEADER_BYTES,
        PLANK_CLIPBOARD_FILE_CONTROL_PAYLOAD_LIMIT, &decoded));
    assert(decoded.type == PLANK_CLIPBOARD_FILE_OFFER_BEGIN);
    assert(decoded.session_epoch == 7 && decoded.sequence == 1);

    for (unsigned i = 0; i < PLANK_CLIPBOARD_FILE_HEADER_BYTES; ++i) {
        uint8_t saved = record[i];
        record[i] ^= 0x80;
        if (i < 16 || i >= 48)
            assert(!plank_clipboard_file_decode(record,
                PLANK_CLIPBOARD_FILE_HEADER_BYTES,
                PLANK_CLIPBOARD_FILE_CONTROL_PAYLOAD_LIMIT, &decoded));
        record[i] = saved;
    }

    assert(plank_clipboard_file_header(record, sizeof(record),
        PLANK_CLIPBOARD_FILE_MATERIALIZE,
        PLANK_CLIPBOARD_FILE_FLAG_ALL_ITEMS, 0, offer, 7, 2));
    assert(plank_clipboard_file_decode(record, PLANK_CLIPBOARD_FILE_HEADER_BYTES,
        PLANK_CLIPBOARD_FILE_CONTROL_PAYLOAD_LIMIT, &decoded));
    assert(!plank_clipboard_file_header(record, sizeof(record),
        PLANK_CLIPBOARD_FILE_OFFER_BEGIN,
        PLANK_CLIPBOARD_FILE_FLAG_ALL_ITEMS, 0, offer, 7, 3));

    const uint8_t good_name[] = {'f', 'l', 'a', 'm', 'e', '-', 0xf0, 0x9f, 0x94, 0xa5};
    const uint8_t slash[] = {'a', '/', 'b'};
    const uint8_t nul[] = {'a', 0, 'b'};
    const uint8_t overlong[] = {0xc0, 0xaf};
    assert(plank_clipboard_file_name_valid(good_name, sizeof(good_name)));
    assert(!plank_clipboard_file_name_valid((const uint8_t *)".", 1));
    assert(!plank_clipboard_file_name_valid((const uint8_t *)"..", 2));
    assert(!plank_clipboard_file_name_valid(slash, sizeof(slash)));
    assert(!plank_clipboard_file_name_valid(nul, sizeof(nul)));
    assert(!plank_clipboard_file_name_valid(overlong, sizeof(overlong)));

    uint8_t range[PLANK_CLIPBOARD_FILE_DATA_PREFIX_BYTES + 3] = {0};
    PlankClipboardFileRange decoded_range;
    assert(plank_clipboard_file_range_header(range, 9, 11, 13, 3));
    memcpy(range + PLANK_CLIPBOARD_FILE_DATA_PREFIX_BYTES, "abc", 3);
    assert(plank_clipboard_file_decode_range(range, sizeof(range), 1, &decoded_range));
    assert(decoded_range.materialization_id == 9 && decoded_range.entry_id == 11);
    assert(decoded_range.offset == 13 && decoded_range.size == 3);
    assert(!plank_clipboard_file_decode_range(range, sizeof(range) - 1, 1, &decoded_range));
    assert(plank_clipboard_file_decode_range(range,
        PLANK_CLIPBOARD_FILE_DATA_PREFIX_BYTES, 0, &decoded_range));

    memset(offer, 0, sizeof(offer));
    assert(!plank_clipboard_file_header(record, sizeof(record),
        PLANK_CLIPBOARD_FILE_OFFER_BEGIN, 0, 0, offer, 7, 4));

    puts("Clipboard file wire: framing, names, flags and ranges passed");
}
