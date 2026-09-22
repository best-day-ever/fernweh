/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef PLANK_CLIPBOARD_FILE_WIRE_H
#define PLANK_CLIPBOARD_FILE_WIRE_H

#include "plank_clipboard_wire.h"

#include <stddef.h>
#include <stdint.h>
#include <string.h>

#define PLANK_CLIPBOARD_FILE_MAGIC 0x32464c50u /* ASCII PLF2 on the wire. */
#define PLANK_CLIPBOARD_FILE_VERSION 2u
#define PLANK_CLIPBOARD_FILE_HEADER_BYTES 56u
#define PLANK_CLIPBOARD_FILE_CONTROL_PAYLOAD_LIMIT (64u * 1024u)
#define PLANK_CLIPBOARD_FILE_DATA_BYTES (256u * 1024u)
#define PLANK_CLIPBOARD_FILE_DATA_PREFIX_BYTES 32u
#define PLANK_CLIPBOARD_FILE_MAX_NAME_BYTES 255u
#define PLANK_CLIPBOARD_FILE_MAX_TOP_LEVEL 1024u
#define PLANK_CLIPBOARD_FILE_MAX_ENTRIES 100000u
#define PLANK_CLIPBOARD_FILE_MAX_DEPTH 64u
#define PLANK_CLIPBOARD_FILE_MAX_MANIFEST_BYTES (32u * 1024u * 1024u)
#define PLANK_CLIPBOARD_FILE_MAX_OUTSTANDING_BYTES (8u * 1024u * 1024u)

#define PLANK_CLIPBOARD_FILE_FLAG_ALL_ITEMS 0x00000001u

typedef enum PlankClipboardFileRecordType {
    PLANK_CLIPBOARD_FILE_OFFER_BEGIN = 1,
    PLANK_CLIPBOARD_FILE_OFFER_ENTRY = 2,
    PLANK_CLIPBOARD_FILE_OFFER_END = 3,
    PLANK_CLIPBOARD_FILE_OFFER_ACK = 4,
    PLANK_CLIPBOARD_FILE_MATERIALIZE = 5,
    PLANK_CLIPBOARD_FILE_READ_REQUEST = 6,
    PLANK_CLIPBOARD_FILE_DATA = 7,
    PLANK_CLIPBOARD_FILE_ENTRY_COMPLETE = 8,
    PLANK_CLIPBOARD_FILE_TRANSFER_READY = 9,
    PLANK_CLIPBOARD_FILE_REVOKE = 10,
    PLANK_CLIPBOARD_FILE_CANCEL = 11,
    PLANK_CLIPBOARD_FILE_ERROR = 12,
} PlankClipboardFileRecordType;

typedef enum PlankClipboardFileDirection {
    PLANK_CLIPBOARD_FILE_CLIENT_TO_HOST = 1,
    PLANK_CLIPBOARD_FILE_HOST_TO_CLIENT = 2,
} PlankClipboardFileDirection;

typedef enum PlankClipboardFileEntryKind {
    PLANK_CLIPBOARD_FILE_REGULAR = 1,
    PLANK_CLIPBOARD_FILE_DIRECTORY = 2,
} PlankClipboardFileEntryKind;

typedef struct PlankClipboardFileRecord {
    uint16_t type;
    uint32_t flags;
    uint32_t payload_size;
    uint8_t offer_id[16];
    uint64_t session_epoch;
    uint64_t sequence;
    const uint8_t *payload;
} PlankClipboardFileRecord;

typedef struct PlankClipboardFileRange {
    uint64_t materialization_id;
    uint64_t entry_id;
    uint64_t offset;
    uint32_t size;
    const uint8_t *bytes;
} PlankClipboardFileRange;

static inline int plank_clipboard_file_offer_id_valid(const uint8_t id[16]) {
    uint8_t combined = 0;
    if (!id) return 0;
    for (unsigned i = 0; i < 16; ++i) combined |= id[i];
    return combined != 0;
}

static inline uint32_t plank_clipboard_file_allowed_flags(uint16_t type) {
    return type == PLANK_CLIPBOARD_FILE_MATERIALIZE ?
        PLANK_CLIPBOARD_FILE_FLAG_ALL_ITEMS : 0u;
}

static inline int plank_clipboard_file_type_valid(uint16_t type) {
    return type >= PLANK_CLIPBOARD_FILE_OFFER_BEGIN &&
           type <= PLANK_CLIPBOARD_FILE_ERROR;
}

static inline int plank_clipboard_file_decode(
        const uint8_t *record, size_t record_size, uint32_t payload_limit,
        PlankClipboardFileRecord *out) {
    if (!record || !out || record_size < PLANK_CLIPBOARD_FILE_HEADER_BYTES) return 0;

    PlankClipboardFileRecord decoded;
    const uint16_t version = (uint16_t)plank_clipboard_read_le(record + 4, 2);
    decoded.type = (uint16_t)plank_clipboard_read_le(record + 6, 2);
    decoded.flags = (uint32_t)plank_clipboard_read_le(record + 8, 4);
    decoded.payload_size = (uint32_t)plank_clipboard_read_le(record + 12, 4);
    memcpy(decoded.offer_id, record + 16, sizeof(decoded.offer_id));
    decoded.session_epoch = plank_clipboard_read_le(record + 32, 8);
    decoded.sequence = plank_clipboard_read_le(record + 40, 8);
    decoded.payload = record + PLANK_CLIPBOARD_FILE_HEADER_BYTES;

    if (plank_clipboard_read_le(record, 4) != PLANK_CLIPBOARD_FILE_MAGIC ||
        version != PLANK_CLIPBOARD_FILE_VERSION ||
        !plank_clipboard_file_type_valid(decoded.type) ||
        (decoded.flags & ~plank_clipboard_file_allowed_flags(decoded.type)) != 0 ||
        decoded.payload_size > payload_limit ||
        record_size != PLANK_CLIPBOARD_FILE_HEADER_BYTES + decoded.payload_size ||
        !plank_clipboard_file_offer_id_valid(decoded.offer_id) ||
        decoded.session_epoch == 0 || decoded.sequence == 0 ||
        plank_clipboard_read_le(record + 48, 8) != 0) {
        return 0;
    }
    *out = decoded;
    return 1;
}

static inline int plank_clipboard_file_header(
        uint8_t *record, size_t capacity, uint16_t type, uint32_t flags,
        uint32_t payload_size, const uint8_t offer_id[16],
        uint64_t session_epoch, uint64_t sequence) {
    if (!record || capacity < PLANK_CLIPBOARD_FILE_HEADER_BYTES ||
        !plank_clipboard_file_type_valid(type) ||
        (flags & ~plank_clipboard_file_allowed_flags(type)) != 0 ||
        !plank_clipboard_file_offer_id_valid(offer_id) ||
        session_epoch == 0 || sequence == 0) return 0;

    plank_clipboard_write_le(record, PLANK_CLIPBOARD_FILE_MAGIC, 4);
    plank_clipboard_write_le(record + 4, PLANK_CLIPBOARD_FILE_VERSION, 2);
    plank_clipboard_write_le(record + 6, type, 2);
    plank_clipboard_write_le(record + 8, flags, 4);
    plank_clipboard_write_le(record + 12, payload_size, 4);
    memcpy(record + 16, offer_id, 16);
    plank_clipboard_write_le(record + 32, session_epoch, 8);
    plank_clipboard_write_le(record + 40, sequence, 8);
    plank_clipboard_write_le(record + 48, 0, 8);
    return 1;
}

static inline int plank_clipboard_file_name_valid(const uint8_t *name, size_t size) {
    if (!name || size == 0 || size > PLANK_CLIPBOARD_FILE_MAX_NAME_BYTES ||
        (size == 1 && name[0] == '.') ||
        (size == 2 && name[0] == '.' && name[1] == '.')) return 0;
    for (size_t i = 0; i < size; ++i) if (name[i] == '/' || name[i] == 0) return 0;
    return plank_clipboard_valid_text(name, size);
}

static inline int plank_clipboard_file_decode_range(
        const uint8_t *payload, size_t payload_size, int with_data,
        PlankClipboardFileRange *out) {
    if (!payload || !out || payload_size < PLANK_CLIPBOARD_FILE_DATA_PREFIX_BYTES) return 0;
    PlankClipboardFileRange decoded;
    decoded.materialization_id = plank_clipboard_read_le(payload, 8);
    decoded.entry_id = plank_clipboard_read_le(payload + 8, 8);
    decoded.offset = plank_clipboard_read_le(payload + 16, 8);
    decoded.size = (uint32_t)plank_clipboard_read_le(payload + 24, 4);
    decoded.bytes = payload + PLANK_CLIPBOARD_FILE_DATA_PREFIX_BYTES;
    if (!decoded.materialization_id || !decoded.entry_id || !decoded.size ||
        decoded.size > PLANK_CLIPBOARD_FILE_DATA_BYTES ||
        decoded.offset > UINT64_MAX - decoded.size ||
        plank_clipboard_read_le(payload + 28, 4) != 0 ||
        payload_size != PLANK_CLIPBOARD_FILE_DATA_PREFIX_BYTES +
            (with_data ? decoded.size : 0u)) return 0;
    *out = decoded;
    return 1;
}

static inline int plank_clipboard_file_range_header(
        uint8_t payload[PLANK_CLIPBOARD_FILE_DATA_PREFIX_BYTES],
        uint64_t materialization_id, uint64_t entry_id,
        uint64_t offset, uint32_t size) {
    if (!payload || !materialization_id || !entry_id || !size ||
        size > PLANK_CLIPBOARD_FILE_DATA_BYTES || offset > UINT64_MAX - size) return 0;
    plank_clipboard_write_le(payload, materialization_id, 8);
    plank_clipboard_write_le(payload + 8, entry_id, 8);
    plank_clipboard_write_le(payload + 16, offset, 8);
    plank_clipboard_write_le(payload + 24, size, 4);
    plank_clipboard_write_le(payload + 28, 0, 4);
    return 1;
}

#endif
