/* SPDX-License-Identifier: AGPL-3.0-or-later */
#ifndef PLANK_TRANSPORT_POINTER_H
#define PLANK_TRANSPORT_POINTER_H

#include "plank_transport_input.h"
#include <math.h>

/* A pointer transaction shares the ordered native input stream with buttons.
 * MOVE is an unaccelerated displacement in encoded-frame pixels, never a
 * sampled Host cursor position. END_ACK is reliable and belongs to one epoch.
 */
#define PLANK_POINTER_CAPTURE_FEATURE 0x40000000u
#define PLANK_POINTER_PACKET_SIZE 32u
#define PLANK_POINTER_VERSION 1u

enum PlankPointerOperation {
    PLANK_POINTER_BEGIN = 1,
    PLANK_POINTER_MOVE = 2,
    PLANK_POINTER_END = 3,
    PLANK_POINTER_CANCEL = 4,
    PLANK_POINTER_END_ACK = 5,
};

typedef struct PlankPointerPacket {
    uint8_t operation;
    uint64_t epoch;
    uint64_t sequence;
    uint16_t width, height;
    float x, y;
} PlankPointerPacket;

static inline void plank_pointer_write_u64(uint8_t *out, uint64_t value) {
    plank_transport_input_write_u32(out, (uint32_t)(value >> 32));
    plank_transport_input_write_u32(out + 4, (uint32_t)value);
}
static inline uint64_t plank_pointer_read_u64(const uint8_t *in) {
    return ((uint64_t)plank_transport_input_read_u32(in) << 32) |
           plank_transport_input_read_u32(in + 4);
}
static inline void plank_pointer_encode(uint8_t out[PLANK_POINTER_PACKET_SIZE],
                                       const PlankPointerPacket *p) {
    out[0] = PLANK_POINTER_VERSION; out[1] = p->operation;
    out[2] = out[3] = 0;
    plank_pointer_write_u64(out + 4, p->epoch);
    plank_pointer_write_u64(out + 12, p->sequence);
    plank_transport_input_write_u16(out + 20, p->width);
    plank_transport_input_write_u16(out + 22, p->height);
    plank_transport_input_write_float(out + 24, p->x);
    plank_transport_input_write_float(out + 28, p->y);
}
static inline int plank_pointer_decode(const uint8_t *in, size_t size,
                                      PlankPointerPacket *p) {
    if (!in || !p || size != PLANK_POINTER_PACKET_SIZE ||
        in[0] != PLANK_POINTER_VERSION || in[2] || in[3] ||
        in[1] < PLANK_POINTER_BEGIN || in[1] > PLANK_POINTER_END_ACK) return -1;
    p->operation = in[1];
    p->epoch = plank_pointer_read_u64(in + 4);
    p->sequence = plank_pointer_read_u64(in + 12);
    p->width = plank_transport_input_read_u16(in + 20);
    p->height = plank_transport_input_read_u16(in + 22);
    p->x = plank_transport_input_read_float(in + 24);
    p->y = plank_transport_input_read_float(in + 28);
    if (!p->epoch || !p->sequence || !p->width || !p->height ||
        p->width > 32767 || p->height > 32767 || !isfinite(p->x) || !isfinite(p->y)) return -1;
    if (p->operation == PLANK_POINTER_BEGIN || p->operation == PLANK_POINTER_END_ACK) {
        if (p->x < 0 || p->x >= p->width || p->y < 0 || p->y >= p->height) return -1;
    } else if (p->operation == PLANK_POINTER_MOVE) {
        if (fabsf(p->x) > 32767 || fabsf(p->y) > 32767) return -1;
    } else if (p->x != 0 || p->y != 0) return -1;
    return 0;
}
#endif
