// SPDX-License-Identifier: GPL-3.0-or-later
#include "../../probes/macos/opus-energy-check.h"
#include <assert.h>
#include <stdio.h>
int main(void) {
    // Correct tone must pass; silence and swapped stereo frequencies must fail.
    for (unsigned mode = 0; mode < 4; ++mode) {
        PLANKOpusEnergyCheck check; assert(PLANKOpusEnergyInit(&check));
        int error; OpusEncoder *encoder = opus_encoder_create(48000, 2, OPUS_APPLICATION_AUDIO, &error);
        assert(encoder && error == OPUS_OK);
        assert(opus_encoder_ctl(encoder, OPUS_SET_BITRATE(96000)) == OPUS_OK);
        for (unsigned packetIndex = 0; packetIndex < 800; ++packetIndex) {
            float pcm[480]; unsigned char packet[4096];
            for (unsigned i = 0; i < 240; ++i) for (unsigned channel = 0; channel < 2; ++channel) {
                unsigned frequency = (channel ^ (mode == 2)) ? 880 : 440;
                pcm[i * 2 + channel] = mode == 1 ? 0 : mode == 3 ?
                    0.05f * (sin(2 * M_PI * 440 * (packetIndex * 240 + i) / 48000) +
                        sin(2 * M_PI * 880 * (packetIndex * 240 + i) / 48000)) :
                    0.1f * sin(2 * M_PI * frequency * (packetIndex * 240 + i) / 48000);
            }
            int bytes = opus_encode_float(encoder, pcm, 240, packet, sizeof(packet));
            assert(bytes > 0 && PLANKOpusEnergyDecode(&check, packet, (size_t)bytes));
        }
        assert(PLANKOpusEnergyPassed(&check) == (mode == 0));
        if (mode == 3) for (unsigned channel = 0; channel < 2; ++channel)
            assert(check.bestFraction[channel] > 0.4 && check.bestOtherFraction[channel] > 0.4);
        assert(!PLANKOpusEnergyDecode(&check, (const unsigned char *)"", 0));
        printf("audio_energy_fixture mode=%u decoded_frames=%llu qualified_windows=%u\n", mode,
            (unsigned long long)check.frames, check.qualifiedWindows);
        opus_encoder_destroy(encoder); PLANKOpusEnergyDestroy(&check);
    }
    puts("audio_energy_check=pass expected_tones=1 silence_rejected=1 swapped_channels_rejected=1 mono_mix_rejected=1");
}
