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

    uint8_t begin_payload[PLANK_CLIPBOARD_FILE_OFFER_BEGIN_BYTES];
    PlankClipboardFileOfferBegin begin = {
        PLANK_CLIPBOARD_FILE_CLIENT_TO_HOST, 17, 2, 3, 987654321
    };
    PlankClipboardFileOfferBegin decoded_begin;
    assert(plank_clipboard_file_offer_begin_encode(begin_payload, &begin));
    assert(plank_clipboard_file_offer_begin_decode(
        begin_payload, sizeof(begin_payload), &decoded_begin));
    assert(decoded_begin.direction == begin.direction);
    assert(decoded_begin.clipboard_generation == 17);
    assert(decoded_begin.top_level_count == 2 && decoded_begin.entry_count == 3);
    assert(decoded_begin.total_bytes == 987654321);
    begin_payload[1] = 1;
    assert(!plank_clipboard_file_offer_begin_decode(
        begin_payload, sizeof(begin_payload), &decoded_begin));
    begin_payload[1] = 0;

    uint8_t entry_payload[PLANK_CLIPBOARD_FILE_OFFER_ENTRY_PREFIX_BYTES +
                          sizeof(good_name)];
    size_t entry_payload_size = 0;
    PlankClipboardFileOfferEntry entry = {
        3, 1, PLANK_CLIPBOARD_FILE_REGULAR, 0700, 1234, -17,
        good_name, sizeof(good_name)
    };
    PlankClipboardFileOfferEntry decoded_entry;
    assert(plank_clipboard_file_offer_entry_encode(
        entry_payload, sizeof(entry_payload), &entry, &entry_payload_size));
    assert(entry_payload_size == sizeof(entry_payload));
    assert(plank_clipboard_file_offer_entry_decode(
        entry_payload, entry_payload_size, &decoded_entry));
    assert(decoded_entry.entry_id == 3 && decoded_entry.parent_id == 1);
    assert(decoded_entry.kind == PLANK_CLIPBOARD_FILE_REGULAR);
    assert(decoded_entry.mode == 0700 && decoded_entry.size == 1234);
    assert(decoded_entry.mtime_ns == -17);
    assert(decoded_entry.name_size == sizeof(good_name));
    assert(!memcmp(decoded_entry.name, good_name, sizeof(good_name)));
    entry_payload[17] = 1;
    assert(!plank_clipboard_file_offer_entry_decode(
        entry_payload, entry_payload_size, &decoded_entry));
    entry_payload[17] = 0;

    uint8_t ack_payload[PLANK_CLIPBOARD_FILE_OFFER_ACK_BYTES];
    PlankClipboardFileOfferAck ack;
    assert(plank_clipboard_file_offer_ack_encode(
        ack_payload, PLANK_CLIPBOARD_FILE_DATA_BYTES,
        PLANK_CLIPBOARD_FILE_MAX_OUTSTANDING_BYTES));
    assert(plank_clipboard_file_offer_ack_decode(ack_payload, sizeof(ack_payload), &ack));
    assert(ack.chunk_size == PLANK_CLIPBOARD_FILE_DATA_BYTES);
    assert(ack.outstanding_window == PLANK_CLIPBOARD_FILE_MAX_OUTSTANDING_BYTES);
    assert(!plank_clipboard_file_offer_ack_encode(
        ack_payload, PLANK_CLIPBOARD_FILE_DATA_BYTES + 1,
        PLANK_CLIPBOARD_FILE_MAX_OUTSTANDING_BYTES));

    uint8_t complete_payload[PLANK_CLIPBOARD_FILE_ENTRY_COMPLETE_BYTES];
    PlankClipboardFileEntryComplete complete = {9, 3, 1234, {0}};
    for (unsigned i = 0; i < sizeof(complete.sha256); ++i) complete.sha256[i] = (uint8_t)i;
    PlankClipboardFileEntryComplete decoded_complete;
    assert(plank_clipboard_file_entry_complete_encode(complete_payload, &complete));
    assert(plank_clipboard_file_entry_complete_decode(
        complete_payload, sizeof(complete_payload), &decoded_complete));
    assert(decoded_complete.materialization_id == 9 && decoded_complete.entry_id == 3);
    assert(decoded_complete.size == 1234);
    assert(!memcmp(decoded_complete.sha256, complete.sha256, sizeof(complete.sha256)));

    uint8_t terminal_payload[PLANK_CLIPBOARD_FILE_TERMINAL_BYTES];
    PlankClipboardFileTerminal terminal = {
        9, PLANK_CLIPBOARD_FILE_REASON_NO_SPACE, 28
    };
    PlankClipboardFileTerminal decoded_terminal;
    assert(plank_clipboard_file_terminal_encode(terminal_payload, &terminal));
    assert(plank_clipboard_file_terminal_decode(
        terminal_payload, sizeof(terminal_payload), &decoded_terminal));
    assert(decoded_terminal.materialization_id == 9);
    assert(decoded_terminal.reason == PLANK_CLIPBOARD_FILE_REASON_NO_SPACE);
    assert(decoded_terminal.detail == 28);
    terminal_payload[10] = 1;
    assert(!plank_clipboard_file_terminal_decode(
        terminal_payload, sizeof(terminal_payload), &decoded_terminal));

    memset(offer, 0, sizeof(offer));
    assert(!plank_clipboard_file_header(record, sizeof(record),
        PLANK_CLIPBOARD_FILE_OFFER_BEGIN, 0, 0, offer, 7, 4));

    puts("Clipboard file wire: framing, manifests, integrity and ranges passed");
}
