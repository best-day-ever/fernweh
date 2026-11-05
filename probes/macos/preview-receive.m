// SPDX-License-Identifier: GPL-3.0-or-later
// Loopback and authorized remote qualification receiver. Launch secret arrives only over stdin.
#import <Foundation/Foundation.h>
#import <CoreMedia/CoreMedia.h>
#import "hevc-decode-check.h"
#ifdef PLANK_PREVIEW_AUDIO_ENERGY
#include "opus-energy-check.h"
#endif
#include "plank_transport.h"
#include "plank_transport_control.h"
#include <unistd.h>
#include <sys/resource.h>
#include <time.h>
#include <math.h>

#define REQUIRE(x) do { if (!(x)) { fprintf(stderr, "preview receiver check failed at line %d\n", __LINE__); return 1; } } while (0)
int main(int argc, const char **argv) {
    unsigned seconds = 3;
    BOOL media = YES;
    BOOL remote = argc >= 5 && !strcmp(argv[2], "--remote");
    BOOL requireAudio = YES, checkAudioEnergy = NO;
    const char *address = "127.0.0.1";
    BOOL waitShared = argc == 3 && !strcmp(argv[2], "--shared-no-media");
    BOOL takeover = argc == 3 && !strcmp(argv[2], "--wait-takeover");
    if (remote) {
        if (argc != 5 && (argc != 6 || (strcmp(argv[5], "--audio-optional") && strcmp(argv[5], "--audio-energy")))) return 2;
        checkAudioEnergy = argc == 6 && !strcmp(argv[5], "--audio-energy");
#ifndef PLANK_PREVIEW_AUDIO_ENERGY
        if (checkAudioEnergy) { fputs("audio energy support was not built\n", stderr); return 2; }
#endif
        address = argv[3]; requireAudio = argc != 6 || checkAudioEnergy;
        char *end = NULL; unsigned long value = strtoul(argv[4], &end, 10);
        if (!end || *end || value < 3 || value > 300) return 2;
        seconds = (unsigned)value;
    } else if (takeover) { media = NO; seconds = 30; }
    else if (argc == 3 && (!strcmp(argv[2], "--no-media") || waitShared)) media = NO;
    else if (argc == 4 && !strcmp(argv[2], "--seconds")) {
        char *end = NULL; unsigned long value = strtoul(argv[3], &end, 10);
        if (!end || *end || value < 3 || value > 30) return 2;
        seconds = (unsigned)value;
    } else if (argc != 2) return 2;
    alarm(seconds + 15);
    struct rlimit core = {0, 0}; REQUIRE(!setrlimit(RLIMIT_CORE, &core));
    @autoreleasepool {
        uint8_t input[32769]; size_t used = 0;
        for (;;) {
            ssize_t size = read(STDIN_FILENO, input + used, sizeof(input) - used);
            REQUIRE(size >= 0);
            if (!size) break;
            used += (size_t)size; REQUIRE(used < sizeof(input));
        }
        NSDictionary *launch = [NSJSONSerialization JSONObjectWithData:[NSData dataWithBytes:input length:used] options:0 error:NULL];
        memset(input, 0, sizeof(input));
        REQUIRE([launch isKindOfClass:NSDictionary.class] && [launch[@"state"] isEqual:@"connecting"]);
        NSString *token = launch[@"transport_token"];
        unsigned width = [launch[@"capture"][@"width"] unsignedIntValue];
        unsigned height = [launch[@"capture"][@"height"] unsignedIntValue];
        unsigned port = [launch[@"udp_port"] unsignedIntValue];
        REQUIRE([token isKindOfClass:NSString.class] && token.length == 44 && port > 0 && port <= UINT16_MAX);
        PlankTransportConfig config = {0}; config.struct_size = sizeof(config); config.abi_version = PLANK_TRANSPORT_ABI_VERSION;
        config.mode = PLANK_TRANSPORT_MODE_CLIENT; config.session_token = token.UTF8String;
        config.remote_address = [NSString stringWithFormat:@"%s:%u", address, port].UTF8String;
        config.server_name = remote ? "plank-host" : "localhost"; config.certificate_sha256 = argv[1];
        config.handshake_timeout_ms = 5000; config.idle_timeout_ms = 5000; config.keep_alive_interval_ms = 1000;
        config.max_udp_payload_size = [launch[@"max_udp_payload_size"] unsignedIntValue];
        PlankTransportNativeEndpoint *endpoint = NULL;
        REQUIRE(plank_transport_native_endpoint_create(&config, &endpoint) == PLANK_TRANSPORT_OK);
        token = nil; launch = nil;
        REQUIRE(plank_transport_native_endpoint_start(endpoint) == PLANK_TRANSPORT_OK);
        REQUIRE(plank_transport_native_endpoint_wait_ready(endpoint, 5000) == PLANK_TRANSPORT_OK);
        if (takeover) {
            puts("takeover_receiver_ready=1"); fflush(stdout);
            const uint64_t deadline = clock_gettime_nsec_np(CLOCK_MONOTONIC) + 30*NSEC_PER_SEC;
            for (;;) {
                REQUIRE(clock_gettime_nsec_np(CLOCK_MONOTONIC) < deadline);
                uint8_t packet[64]; size_t count = 0;
                int32_t got = plank_transport_native_data_receive(endpoint, packet, sizeof(packet), &count, 1000);
                if (got == PLANK_TRANSPORT_TIMEOUT) continue;
                REQUIRE(got == PLANK_TRANSPORT_OK);
                PlankTransportControlPacket control;
                REQUIRE(!plank_transport_control_decode(packet, count, &control));
                if (control.type == PLANK_TRANSPORT_CONTROL_VIDEO_BITRATE_APPLIED) continue;
                REQUIRE(control.type == PLANK_TRANSPORT_CONTROL_HOST_TERMINATE && control.payload_size == 4);
                REQUIRE(plank_transport_control_read_u32(control.payload) == PLANK_TRANSPORT_TERMINATION_SESSION_TAKEN_OVER);
                break;
            }
            plank_transport_native_endpoint_destroy(endpoint);
            puts("takeover_receiver_terminal=1"); return 0;
        }
        if (waitShared) {
            puts("shared_receiver_ready=1"); fflush(stdout);
            // Test driver disconnects another peer while this endpoint stays up.
            uint64_t deadline = clock_gettime_nsec_np(CLOCK_MONOTONIC) + 8*NSEC_PER_SEC;
            while (clock_gettime_nsec_np(CLOCK_MONOTONIC) < deadline) {
                REQUIRE(plank_transport_native_endpoint_state(endpoint) == PLANK_TRANSPORT_STATE_READY);
                usleep(10000);
            }
        }
        #ifdef PLANK_PREVIEW_AUDIO_ENERGY
        PLANKOpusEnergyCheck audioEnergy;
        if (checkAudioEnergy) REQUIRE(PLANKOpusEnergyInit(&audioEnergy));
#endif
        PLANKHEVCDecodeCheck *decoder = remote ? [PLANKHEVCDecodeCheck new] : nil;
        uint64_t frames = 0, totalBytes = 0, lastPTS = 0, audioPackets = 0, lastAudioPTS = 0;
        double audioMaxAgeMs = 0, firstVideoMs = 0, firstAudioMs = 0;
        if (media) {
            NSMutableData *payload = [NSMutableData dataWithLength:64 * 1024 * 1024];
            uint64_t deadline = clock_gettime_nsec_np(CLOCK_MONOTONIC) + (uint64_t)seconds * NSEC_PER_SEC;
            while (clock_gettime_nsec_np(CLOCK_MONOTONIC) < deadline) {
                PlankTransportNativeVideoFrameInfo info = {0}; info.struct_size = sizeof(info);
                size_t count = 0;
                int32_t result = plank_transport_native_video_receive(endpoint, &info, payload.mutableBytes, payload.length, &count, 5);
                if (result != PLANK_TRANSPORT_OK && result != PLANK_TRANSPORT_TIMEOUT)
                    fprintf(stderr, "media_stopped result=%d state=%u frames=%llu audio_packets=%llu\n", result,
                        plank_transport_native_endpoint_state(endpoint), (unsigned long long)frames, (unsigned long long)audioPackets);
                REQUIRE(result == PLANK_TRANSPORT_OK || result == PLANK_TRANSPORT_TIMEOUT);
                if (result == PLANK_TRANSPORT_OK) {
                    REQUIRE(count >= 6 && info.codec == PLANK_TRANSPORT_NATIVE_VIDEO_CODEC_HEVC);
                    REQUIRE(frames ? info.pts > lastPTS : (info.flags & PLANK_TRANSPORT_NATIVE_VIDEO_FLAG_KEY));
                    static const uint8_t start[] = {0, 0, 0, 1};
                    REQUIRE(!memcmp(payload.bytes, start, 4));
                    if (remote) {
                        REQUIRE([decoder decode:payload.bytes length:count width:width height:height]);
                        if (!frames) { puts("remote_receiver_pixels_ready=1"); fflush(stdout); }
                    }
                    if (!frames) firstVideoMs = info.pts / 90.0;
                    ++frames; totalBytes += count; lastPTS = info.pts;
                }
                // Do not block on video and overflow the independent audio
                // receiver queue while a static desktop produces no new frame.
                for (unsigned i = 0; i < 32; ++i) {
                    uint8_t audio[65536]; size_t size = 0;
                    PlankTransportNativeAudioPacketInfo packet = {0}; packet.struct_size = sizeof(packet);
                    int32_t got = plank_transport_native_audio_receive(endpoint, &packet, audio, sizeof(audio), &size, 0);
                    if (got == PLANK_TRANSPORT_TIMEOUT) break;
                    REQUIRE(got == PLANK_TRANSPORT_OK && size > 0 && packet.frame_samples == 240 && packet.missing_samples == 0);
                    REQUIRE(!audioPackets || packet.pts == lastAudioPTS + 5);
#ifdef PLANK_PREVIEW_AUDIO_ENERGY
                    if (checkAudioEnergy) REQUIRE(PLANKOpusEnergyDecode(&audioEnergy, audio, size));
#endif
                    if (!audioPackets) firstAudioMs = (double)packet.pts;
                    double ageMs = CMTimeGetSeconds(CMClockGetTime(CMClockGetHostTimeClock())) * 1000 - packet.pts;
                    // Monotonic clocks on different machines do not share an epoch.
                    if (!remote) REQUIRE(isfinite(ageMs) && ageMs >= -1 && ageMs < 2000);
                    else ageMs = 0;
                    audioMaxAgeMs = fmax(audioMaxAgeMs, ageMs);
                    ++audioPackets; lastAudioPTS = packet.pts;
                }
            }
            REQUIRE(frames > 0 && (!requireAudio || audioPackets > 100));
            if (audioPackets) REQUIRE(fabs(firstVideoMs - firstAudioMs) < 1000); // same source clock, not a sync-quality gate
        }
        #ifdef PLANK_PREVIEW_AUDIO_ENERGY
        if (checkAudioEnergy) {
            printf("audio_energy decoded_frames=%llu qualified_windows=%u left_rms=%.6f right_rms=%.6f left_tone_fraction=%.6f right_tone_fraction=%.6f left_other_tone_fraction=%.6f right_other_tone_fraction=%.6f\n",
                (unsigned long long)audioEnergy.frames, audioEnergy.qualifiedWindows,
                audioEnergy.bestRMS[0], audioEnergy.bestRMS[1], audioEnergy.bestFraction[0], audioEnergy.bestFraction[1],
                audioEnergy.bestOtherFraction[0], audioEnergy.bestOtherFraction[1]);
            BOOL passed = PLANKOpusEnergyPassed(&audioEnergy);
            PLANKOpusEnergyDestroy(&audioEnergy);
            REQUIRE(passed);
            puts("audio_energy=pass expected_left_hz=440 expected_right_hz=880");
        }
#endif
        uint8_t control[20]; size_t count = 0;
        uint32_t target = 111500;
        REQUIRE(!plank_transport_control_encode(PLANK_TRANSPORT_CONTROL_SET_VIDEO_BITRATE, &target, 1, control, sizeof(control), &count));
        REQUIRE(plank_transport_native_data_send(endpoint, control, count) == PLANK_TRANSPORT_OK);
        REQUIRE(plank_transport_native_data_receive(endpoint, control, sizeof(control), &count, 5000) == PLANK_TRANSPORT_OK);
        PlankTransportControlPacket ack;
        REQUIRE(!plank_transport_control_decode(control, count, &ack));
        REQUIRE(ack.type == PLANK_TRANSPORT_CONTROL_VIDEO_BITRATE_APPLIED && ack.payload_size == 12);
        REQUIRE(plank_transport_control_read_u32(ack.payload) == target && plank_transport_control_read_u32(ack.payload + 4) == target);
        REQUIRE(plank_transport_control_read_u32(ack.payload + 8) >= target);
        REQUIRE(!plank_transport_control_encode(PLANK_TRANSPORT_CONTROL_CLIENT_DISCONNECT, NULL, 0, control, sizeof(control), &count));
        REQUIRE(plank_transport_native_data_send(endpoint, control, count) == PLANK_TRANSPORT_OK);
        uint64_t deadline = clock_gettime_nsec_np(CLOCK_MONOTONIC) + 3 * NSEC_PER_SEC;
        while (plank_transport_native_endpoint_state(endpoint) == PLANK_TRANSPORT_STATE_READY &&
            clock_gettime_nsec_np(CLOCK_MONOTONIC) < deadline) usleep(10000);
        REQUIRE(plank_transport_native_endpoint_state(endpoint) != PLANK_TRANSPORT_STATE_READY);
        plank_transport_native_endpoint_destroy(endpoint);
        if (remote) printf("remote_decode=pass decoded_frames=%lu pixel_format=%u\n",
            (unsigned long)decoder.decodedFrames, (unsigned)decoder.pixelFormat);
        printf("macos_preview_receiver=pass media=%d seconds=%u negotiated_pixels=%ux%u frames=%llu bytes=%llu audio_packets=%llu audio_max_source_age_ms=%.3f first_video_minus_audio_ms=%.3f bitrate_ack=1 disconnected=1\n",
            media, seconds, width, height, (unsigned long long)frames, (unsigned long long)totalBytes,
            (unsigned long long)audioPackets, audioMaxAgeMs, audioPackets ? firstVideoMs - firstAudioMs : 0);
    }
}
