// SPDX-License-Identifier: GPL-3.0-or-later
// Acceptance-only expected-tone check. Retains aggregate statistics, never PCM.
#pragma once
#include <opus/opus_multistream.h>
#include <math.h>
#include <stdint.h>
#include <string.h>

typedef struct {
    OpusMSDecoder *decoder;
    uint64_t frames;
    unsigned windowFrames, qualifiedWindows;
    double energy[2], real[2][2], imaginary[2][2], bestRMS[2], bestFraction[2], bestOtherFraction[2];
} PLANKOpusEnergyCheck;
static inline int PLANKOpusEnergyInit(PLANKOpusEnergyCheck *check) {
    memset(check, 0, sizeof(*check));
    const unsigned char mapping[] = {0, 1}; int error;
    check->decoder = opus_multistream_decoder_create(48000, 2, 1, 1, mapping, &error);
    return check->decoder && error == OPUS_OK;
}
static inline int PLANKOpusEnergyDecode(PLANKOpusEnergyCheck *check, const unsigned char *packet, size_t bytes) {
    if (!check->decoder || !bytes || bytes > 65536 ||
        opus_packet_get_nb_samples(packet, (opus_int32)bytes, 48000) != 240) return 0;
    float pcm[480];
    if (opus_multistream_decode_float(check->decoder, packet, (opus_int32)bytes, pcm, 240, 0) != 240) return 0;
    for (unsigned i = 0; i < 240; ++i) {
        for (unsigned channel = 0; channel < 2; ++channel) {
            double value = pcm[i * 2 + channel];
            if (!isfinite(value) || fabs(value) > 2) return 0;
            check->energy[channel] += value * value;
            for (unsigned tone = 0; tone < 2; ++tone) {
                double phase = 2 * M_PI * (tone ? 880 : 440) * check->windowFrames / 48000;
                check->real[channel][tone] += value * cos(phase);
                check->imaginary[channel][tone] += value * sin(phase);
            }
        }
        ++check->frames;
        if (++check->windowFrames == 48000) {
            int qualified = 1;
            for (unsigned channel = 0; channel < 2; ++channel) {
                double rms = sqrt(check->energy[channel] / 48000);
                double fractions[2];
                for (unsigned tone = 0; tone < 2; ++tone)
                    fractions[tone] = 2 * (check->real[channel][tone] * check->real[channel][tone] +
                        check->imaginary[channel][tone] * check->imaginary[channel][tone]) / fmax(48000 * check->energy[channel], 1e-30);
                double fraction = fractions[channel];
                if (fraction > check->bestFraction[channel]) {
                    check->bestFraction[channel] = fraction; check->bestRMS[channel] = rms;
                    check->bestOtherFraction[channel] = fractions[1 - channel];
                }
                qualified &= rms > 0.01 && fraction > 0.8;
                check->energy[channel] = 0;
                for (unsigned tone = 0; tone < 2; ++tone)
                    check->real[channel][tone] = check->imaginary[channel][tone] = 0;
            }
            check->qualifiedWindows += qualified;
            check->windowFrames = 0;
        }
    }
    return 1;
}
static inline int PLANKOpusEnergyPassed(const PLANKOpusEnergyCheck *check) { return check->qualifiedWindows >= 2; }
static inline void PLANKOpusEnergyDestroy(PLANKOpusEnergyCheck *check) {
    if (check->decoder) opus_multistream_decoder_destroy(check->decoder);
    check->decoder = NULL;
}
