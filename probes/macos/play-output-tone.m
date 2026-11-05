// SPDX-License-Identifier: GPL-3.0-or-later
// Eight-second generated stereo tone to PLANK Output only. No default changes.
#import <Foundation/Foundation.h>
#import <AudioToolbox/AudioToolbox.h>
#import <CoreAudio/CoreAudio.h>
#import "../../apps/host/macos/audio-device/output-format.h"
#include <stdatomic.h>
#include <math.h>
#include <unistd.h>

enum { Rate = 48000, FramesPerBuffer = 4800, TotalFrames = 8 * Rate };
typedef struct { unsigned generated, queued; atomic_bool done, failed; } Tone;
static void fill(Tone *tone, AudioQueueRef queue, AudioQueueBufferRef buffer) {
    unsigned frames = MIN(FramesPerBuffer, TotalFrames - tone->generated);
    if (!frames) { if (!tone->queued) atomic_store(&tone->done, true); return; }
    float *pcm = buffer->mAudioData;
    for (unsigned i = 0; i < frames; ++i) for (unsigned channel = 0; channel < 2; ++channel)
        pcm[i * 2 + channel] = 0.1f * sin(2 * M_PI * (channel ? 880 : 440) * (tone->generated + i) / Rate);
    tone->generated += frames; buffer->mAudioDataByteSize = frames * 2 * sizeof(float);
    if (AudioQueueEnqueueBuffer(queue, buffer, 0, NULL)) atomic_store(&tone->failed, true);
    else ++tone->queued;
}
static void consumed(void *context, AudioQueueRef queue, AudioQueueBufferRef buffer) {
    Tone *tone = context;
    if (tone->queued) --tone->queued;
    fill(tone, queue, buffer);
}
int main(void) {
    @autoreleasepool {
        CFStringRef uid = CFSTR(PLANK_OUTPUT_DEVICE_UID);
        AudioObjectID device = kAudioObjectUnknown;
        AudioValueTranslation translation = {&uid, sizeof(uid), &device, sizeof(device)};
        UInt32 bytes = sizeof(translation);
        AudioObjectPropertyAddress property = {kAudioHardwarePropertyDeviceForUID,
            kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
        if (AudioObjectGetPropertyData(kAudioObjectSystemObject, &property, 0, NULL, &bytes, &translation) ||
            device == kAudioObjectUnknown) return 1;
        AudioStreamBasicDescription format = {0};
        format.mFormatID = kAudioFormatLinearPCM; format.mSampleRate = Rate;
        format.mFormatFlags = kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked;
        format.mChannelsPerFrame = 2; format.mBitsPerChannel = 32; format.mFramesPerPacket = 1;
        format.mBytesPerFrame = format.mBytesPerPacket = 2 * sizeof(float);
        Tone tone = {0}; atomic_init(&tone.done, false); atomic_init(&tone.failed, false);
        AudioQueueRef queue = NULL;
        OSStatus status = AudioQueueNewOutput(&format, consumed, &tone, NULL, NULL, 0, &queue);
        if (status || !queue) return 1;
        status = AudioQueueSetProperty(queue, kAudioQueueProperty_CurrentDevice, &uid, sizeof(uid));
        for (unsigned i = 0; !status && i < 3; ++i) {
            AudioQueueBufferRef buffer = NULL;
            status = AudioQueueAllocateBuffer(queue, FramesPerBuffer * 2 * sizeof(float), &buffer);
            if (!status) fill(&tone, queue, buffer);
        }
        if (!status) status = AudioQueueStart(queue, NULL);
        for (unsigned step = 0; !status && !atomic_load(&tone.failed) && !atomic_load(&tone.done) && step < 1200; ++step)
            usleep(10000);
        BOOL passed = !status && !atomic_load(&tone.failed) && atomic_load(&tone.done);
        AudioQueueStop(queue, true); AudioQueueDispose(queue, true);
        printf("explicit_output_tone=%s seconds=8 left_hz=440 right_hz=880 default_route_changed=0\n", passed ? "pass" : "fail");
        return passed ? 0 : 1;
    }
}
