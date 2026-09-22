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

#define PLANK_CLIPBOARD_FILE_OFFER_BEGIN_BYTES 40u
#define PLANK_CLIPBOARD_FILE_OFFER_ENTRY_PREFIX_BYTES 40u
#define PLANK_CLIPBOARD_FILE_OFFER_ACK_BYTES 8u
#define PLANK_CLIPBOARD_FILE_MATERIALIZE_BYTES 8u
#define PLANK_CLIPBOARD_FILE_ENTRY_COMPLETE_BYTES 56u
#define PLANK_CLIPBOARD_FILE_TERMINAL_BYTES 16u

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

typedef enum PlankClipboardFileReason {
    PLANK_CLIPBOARD_FILE_REASON_UNSUPPORTED = 1,
    PLANK_CLIPBOARD_FILE_REASON_POLICY_DENIED = 2,
    PLANK_CLIPBOARD_FILE_REASON_OFFER_REPLACED = 3,
    PLANK_CLIPBOARD_FILE_REASON_LIMIT_EXCEEDED = 4,
    PLANK_CLIPBOARD_FILE_REASON_INVALID_MANIFEST = 5,
    PLANK_CLIPBOARD_FILE_REASON_UNSUPPORTED_ENTRY_TYPE = 6,
    PLANK_CLIPBOARD_FILE_REASON_SOURCE_UNAVAILABLE = 7,
    PLANK_CLIPBOARD_FILE_REASON_SOURCE_CHANGED = 8,
    PLANK_CLIPBOARD_FILE_REASON_DESTINATION_UNAVAILABLE = 9,
    PLANK_CLIPBOARD_FILE_REASON_NO_SPACE = 10,
    PLANK_CLIPBOARD_FILE_REASON_INTEGRITY_FAILED = 11,
    PLANK_CLIPBOARD_FILE_REASON_CANCELLED_BY_USER = 12,
    PLANK_CLIPBOARD_FILE_REASON_SESSION_ENDED = 13,
    PLANK_CLIPBOARD_FILE_REASON_PROTOCOL_ERROR = 14,
    PLANK_CLIPBOARD_FILE_REASON_INTERNAL_ERROR = 15,
} PlankClipboardFileReason;

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

typedef struct PlankClipboardFileOfferBegin {
    uint8_t direction;
    uint64_t clipboard_generation;
    uint32_t top_level_count;
    uint32_t entry_count;
    uint64_t total_bytes;
} PlankClipboardFileOfferBegin;

typedef struct PlankClipboardFileOfferEntry {
    uint64_t entry_id;
    uint64_t parent_id;
    uint8_t kind;
    uint16_t mode;
    uint64_t size;
    int64_t mtime_ns;
    const uint8_t *name;
    uint16_t name_size;
} PlankClipboardFileOfferEntry;

typedef struct PlankClipboardFileOfferAck {
    uint32_t chunk_size;
    uint32_t outstanding_window;
} PlankClipboardFileOfferAck;

typedef struct PlankClipboardFileEntryComplete {
    uint64_t materialization_id;
    uint64_t entry_id;
    uint64_t size;
    uint8_t sha256[32];
} PlankClipboardFileEntryComplete;

typedef struct PlankClipboardFileTerminal {
    uint64_t materialization_id;
    uint16_t reason;
    uint32_t detail;
} PlankClipboardFileTerminal;

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

static inline int plank_clipboard_file_direction_valid(uint8_t direction) {
    return direction == PLANK_CLIPBOARD_FILE_CLIENT_TO_HOST ||
           direction == PLANK_CLIPBOARD_FILE_HOST_TO_CLIENT;
}

static inline int plank_clipboard_file_reason_valid(uint16_t reason) {
    return reason >= PLANK_CLIPBOARD_FILE_REASON_UNSUPPORTED &&
           reason <= PLANK_CLIPBOARD_FILE_REASON_INTERNAL_ERROR;
}

static inline int plank_clipboard_file_offer_begin_encode(
        uint8_t payload[PLANK_CLIPBOARD_FILE_OFFER_BEGIN_BYTES],
        const PlankClipboardFileOfferBegin *offer) {
    if (!payload || !offer ||
        !plank_clipboard_file_direction_valid(offer->direction) ||
        !offer->clipboard_generation || !offer->top_level_count ||
        offer->top_level_count > PLANK_CLIPBOARD_FILE_MAX_TOP_LEVEL ||
        !offer->entry_count ||
        offer->entry_count > PLANK_CLIPBOARD_FILE_MAX_ENTRIES ||
        offer->top_level_count > offer->entry_count) return 0;
    memset(payload, 0, PLANK_CLIPBOARD_FILE_OFFER_BEGIN_BYTES);
    payload[0] = offer->direction;
    plank_clipboard_write_le(payload + 8, offer->clipboard_generation, 8);
    plank_clipboard_write_le(payload + 16, offer->top_level_count, 4);
    plank_clipboard_write_le(payload + 20, offer->entry_count, 4);
    plank_clipboard_write_le(payload + 24, offer->total_bytes, 8);
    return 1;
}

static inline int plank_clipboard_file_offer_begin_decode(
        const uint8_t *payload, size_t payload_size,
        PlankClipboardFileOfferBegin *offer) {
    if (!payload || !offer ||
        payload_size != PLANK_CLIPBOARD_FILE_OFFER_BEGIN_BYTES ||
        plank_clipboard_read_le(payload + 1, 7) != 0 ||
        plank_clipboard_read_le(payload + 32, 8) != 0) return 0;
    PlankClipboardFileOfferBegin decoded;
    decoded.direction = payload[0];
    decoded.clipboard_generation = plank_clipboard_read_le(payload + 8, 8);
    decoded.top_level_count = (uint32_t)plank_clipboard_read_le(payload + 16, 4);
    decoded.entry_count = (uint32_t)plank_clipboard_read_le(payload + 20, 4);
    decoded.total_bytes = plank_clipboard_read_le(payload + 24, 8);
    if (!plank_clipboard_file_direction_valid(decoded.direction) ||
        !decoded.clipboard_generation || !decoded.top_level_count ||
        decoded.top_level_count > PLANK_CLIPBOARD_FILE_MAX_TOP_LEVEL ||
        !decoded.entry_count || decoded.entry_count > PLANK_CLIPBOARD_FILE_MAX_ENTRIES ||
        decoded.top_level_count > decoded.entry_count) return 0;
    *offer = decoded;
    return 1;
}

static inline int plank_clipboard_file_offer_entry_encode(
        uint8_t *payload, size_t capacity,
        const PlankClipboardFileOfferEntry *entry, size_t *payload_size) {
    if (!payload || !entry || !payload_size || !entry->entry_id ||
        (entry->kind != PLANK_CLIPBOARD_FILE_REGULAR &&
         entry->kind != PLANK_CLIPBOARD_FILE_DIRECTORY) ||
        (entry->mode & ~0700u) != 0 ||
        (entry->kind == PLANK_CLIPBOARD_FILE_DIRECTORY && entry->size != 0) ||
        !plank_clipboard_file_name_valid(entry->name, entry->name_size) ||
        capacity < PLANK_CLIPBOARD_FILE_OFFER_ENTRY_PREFIX_BYTES + entry->name_size)
        return 0;
    memset(payload, 0, PLANK_CLIPBOARD_FILE_OFFER_ENTRY_PREFIX_BYTES);
    plank_clipboard_write_le(payload, entry->entry_id, 8);
    plank_clipboard_write_le(payload + 8, entry->parent_id, 8);
    payload[16] = entry->kind;
    plank_clipboard_write_le(payload + 18, entry->mode, 2);
    plank_clipboard_write_le(payload + 20, entry->size, 8);
    plank_clipboard_write_le(payload + 28, (uint64_t)entry->mtime_ns, 8);
    plank_clipboard_write_le(payload + 36, entry->name_size, 2);
    memcpy(payload + PLANK_CLIPBOARD_FILE_OFFER_ENTRY_PREFIX_BYTES,
           entry->name, entry->name_size);
    *payload_size = PLANK_CLIPBOARD_FILE_OFFER_ENTRY_PREFIX_BYTES + entry->name_size;
    return 1;
}

static inline int plank_clipboard_file_offer_entry_decode(
        const uint8_t *payload, size_t payload_size,
        PlankClipboardFileOfferEntry *entry) {
    if (!payload || !entry ||
        payload_size < PLANK_CLIPBOARD_FILE_OFFER_ENTRY_PREFIX_BYTES ||
        payload[17] != 0 || plank_clipboard_read_le(payload + 38, 2) != 0)
        return 0;
    PlankClipboardFileOfferEntry decoded;
    decoded.entry_id = plank_clipboard_read_le(payload, 8);
    decoded.parent_id = plank_clipboard_read_le(payload + 8, 8);
    decoded.kind = payload[16];
    decoded.mode = (uint16_t)plank_clipboard_read_le(payload + 18, 2);
    decoded.size = plank_clipboard_read_le(payload + 20, 8);
    decoded.mtime_ns = (int64_t)plank_clipboard_read_le(payload + 28, 8);
    decoded.name_size = (uint16_t)plank_clipboard_read_le(payload + 36, 2);
    decoded.name = payload + PLANK_CLIPBOARD_FILE_OFFER_ENTRY_PREFIX_BYTES;
    if (!decoded.entry_id ||
        (decoded.kind != PLANK_CLIPBOARD_FILE_REGULAR &&
         decoded.kind != PLANK_CLIPBOARD_FILE_DIRECTORY) ||
        (decoded.mode & ~0700u) != 0 ||
        (decoded.kind == PLANK_CLIPBOARD_FILE_DIRECTORY && decoded.size != 0) ||
        payload_size != PLANK_CLIPBOARD_FILE_OFFER_ENTRY_PREFIX_BYTES +
                        decoded.name_size ||
        !plank_clipboard_file_name_valid(decoded.name, decoded.name_size)) return 0;
    *entry = decoded;
    return 1;
}

static inline int plank_clipboard_file_offer_ack_encode(
        uint8_t payload[PLANK_CLIPBOARD_FILE_OFFER_ACK_BYTES],
        uint32_t chunk_size, uint32_t outstanding_window) {
    if (!payload || !chunk_size || chunk_size > PLANK_CLIPBOARD_FILE_DATA_BYTES ||
        chunk_size > outstanding_window ||
        outstanding_window > PLANK_CLIPBOARD_FILE_MAX_OUTSTANDING_BYTES) return 0;
    plank_clipboard_write_le(payload, chunk_size, 4);
    plank_clipboard_write_le(payload + 4, outstanding_window, 4);
    return 1;
}

static inline int plank_clipboard_file_offer_ack_decode(
        const uint8_t *payload, size_t payload_size,
        PlankClipboardFileOfferAck *ack) {
    if (!payload || !ack || payload_size != PLANK_CLIPBOARD_FILE_OFFER_ACK_BYTES)
        return 0;
    PlankClipboardFileOfferAck decoded;
    decoded.chunk_size = (uint32_t)plank_clipboard_read_le(payload, 4);
    decoded.outstanding_window = (uint32_t)plank_clipboard_read_le(payload + 4, 4);
    if (!decoded.chunk_size || decoded.chunk_size > PLANK_CLIPBOARD_FILE_DATA_BYTES ||
        decoded.chunk_size > decoded.outstanding_window ||
        decoded.outstanding_window > PLANK_CLIPBOARD_FILE_MAX_OUTSTANDING_BYTES)
        return 0;
    *ack = decoded;
    return 1;
}

static inline int plank_clipboard_file_materialization_encode(
        uint8_t payload[PLANK_CLIPBOARD_FILE_MATERIALIZE_BYTES],
        uint64_t materialization_id) {
    if (!payload || !materialization_id) return 0;
    plank_clipboard_write_le(payload, materialization_id, 8);
    return 1;
}

static inline int plank_clipboard_file_materialization_decode(
        const uint8_t *payload, size_t payload_size,
        uint64_t *materialization_id) {
    if (!payload || !materialization_id ||
        payload_size != PLANK_CLIPBOARD_FILE_MATERIALIZE_BYTES) return 0;
    *materialization_id = plank_clipboard_read_le(payload, 8);
    return *materialization_id != 0;
}

static inline int plank_clipboard_file_entry_complete_encode(
        uint8_t payload[PLANK_CLIPBOARD_FILE_ENTRY_COMPLETE_BYTES],
        const PlankClipboardFileEntryComplete *complete) {
    if (!payload || !complete || !complete->materialization_id ||
        !complete->entry_id) return 0;
    plank_clipboard_write_le(payload, complete->materialization_id, 8);
    plank_clipboard_write_le(payload + 8, complete->entry_id, 8);
    plank_clipboard_write_le(payload + 16, complete->size, 8);
    memcpy(payload + 24, complete->sha256, 32);
    return 1;
}

static inline int plank_clipboard_file_entry_complete_decode(
        const uint8_t *payload, size_t payload_size,
        PlankClipboardFileEntryComplete *complete) {
    if (!payload || !complete ||
        payload_size != PLANK_CLIPBOARD_FILE_ENTRY_COMPLETE_BYTES) return 0;
    PlankClipboardFileEntryComplete decoded;
    decoded.materialization_id = plank_clipboard_read_le(payload, 8);
    decoded.entry_id = plank_clipboard_read_le(payload + 8, 8);
    decoded.size = plank_clipboard_read_le(payload + 16, 8);
    memcpy(decoded.sha256, payload + 24, 32);
    if (!decoded.materialization_id || !decoded.entry_id) return 0;
    *complete = decoded;
    return 1;
}

static inline int plank_clipboard_file_terminal_encode(
        uint8_t payload[PLANK_CLIPBOARD_FILE_TERMINAL_BYTES],
        const PlankClipboardFileTerminal *terminal) {
    if (!payload || !terminal ||
        !plank_clipboard_file_reason_valid(terminal->reason)) return 0;
    memset(payload, 0, PLANK_CLIPBOARD_FILE_TERMINAL_BYTES);
    plank_clipboard_write_le(payload, terminal->materialization_id, 8);
    plank_clipboard_write_le(payload + 8, terminal->reason, 2);
    plank_clipboard_write_le(payload + 12, terminal->detail, 4);
    return 1;
}

static inline int plank_clipboard_file_terminal_decode(
        const uint8_t *payload, size_t payload_size,
        PlankClipboardFileTerminal *terminal) {
    if (!payload || !terminal || payload_size != PLANK_CLIPBOARD_FILE_TERMINAL_BYTES ||
        plank_clipboard_read_le(payload + 10, 2) != 0) return 0;
    PlankClipboardFileTerminal decoded;
    decoded.materialization_id = plank_clipboard_read_le(payload, 8);
    decoded.reason = (uint16_t)plank_clipboard_read_le(payload + 8, 2);
    decoded.detail = (uint32_t)plank_clipboard_read_le(payload + 12, 4);
    if (!plank_clipboard_file_reason_valid(decoded.reason)) return 0;
    *terminal = decoded;
    return 1;
}

#endif
