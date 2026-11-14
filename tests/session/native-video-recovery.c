// Exercise the production assembler with synthetic Annex-B framing, no decoder,
// display, network or input devices. Keep recovery requests observable.
#define PltGetMillis testGetMillis
#include "VideoFrameAssembler.c"
#undef PltGetMillis
#include <stdio.h>
#include <pthread.h>
#include <stdatomic.h>

#define CHECK(x) do { checks++; if (!(x)) { \
    fprintf(stderr, "line %d: %s\n", __LINE__, #x); exit(1); } } while (0)
static unsigned checks, delivered, keys;
static atomic_uint requests;
static int decoderResult;
static uint64_t fakeNow;
static bool pullMode;
uint64_t testGetMillis(void) { return fakeNow; }
int NegotiatedVideoFormat;
STREAM_CONFIGURATION StreamConfig;
DECODER_RENDERER_CALLBACKS VideoCallbacks;
CONNECTION_LISTENER_CALLBACKS ListenerCallbacks;

static void quietLog(const char* format, ...) { (void)format; }
void LiRequestIdrFrame(void) { requests++; }
void connectionDetectedFrameLoss(uint32_t start, uint32_t end) {
    CHECK(start <= end);
    LiRequestIdrFrame(); // real non-RFI ControlStream behavior
}
void connectionReceivedCompleteFrame(uint32_t frame, bool ltr) { (void)frame; (void)ltr; }
void notifyKeyFrameReceived(void) { keys++; }
static int submit(PDECODE_UNIT unit) {
    CHECK(unit->fullLength > 0);
    CHECK(LiVideoFrameData(unit) == LiVideoBufferData(unit->frameBuffer));
    for (int i = 0; i < PLANK_VIDEO_BUFFER_PADDING; ++i)
        CHECK(LiVideoFrameData(unit)[unit->fullLength + i] == 0);
    delivered++;
    return decoderResult;
}

static const unsigned char h264[] = {
    0,0,0,1,0x67,0xab, 0,0,0,1,0x68,0xab, 0,0,0,1,0x65,0xab
};
static const unsigned char hevc[] = {
    0,0,0,1,0x40,1,0xab, 0,0,0,1,0x42,1,0xab,
    0,0,0,1,0x44,1,0xab, 0,0,0,1,0x26,1,0xab
};
static const unsigned char delta[] = {0,0,0,1,0x02,1,0xab};

static int frame(const unsigned char* data, size_t length, uint32_t number, bool key) {
    int result = LiSubmitPlankVideoFrame(data, (int)length, number,
        key ? PLANK_VIDEO_FRAME_FLAG_KEY : 0, number * 1500, 1);
    if (result == 0 && pullMode) {
        VIDEO_FRAME_HANDLE handle;
        PDECODE_UNIT unit;
        CHECK(LiPollNextVideoFrame(&handle, &unit));
        LiCompleteVideoFrame(handle, submit(unit));
        CHECK(LiGetPendingVideoFrames() == 0);
    }
    return result;
}

static void exercise(int codec, const unsigned char* key, size_t size, size_t configSize) {
    NegotiatedVideoFormat = codec;
    requests = delivered = keys = 0;
    fakeNow = 0;
    decoderResult = DR_OK;
    initializeVideoFrameAssembler();
    CHECK(frame(key, size, 1, true) == 0);
    CHECK(delivered == 1 && requests == 0);
    CHECK(frame(delta, sizeof(delta), 2, false) == 0);
    CHECK(delivered == 2);

    // Sender omitted encoded frames awaiting recovery, or transport lost them.
    // The arriving keyframe itself repairs the reference chain.
    CHECK(frame(key, size, 10, true) == 0);
    CHECK(requests == 0 && delivered == 3);
    CHECK(frame(delta, sizeof(delta), 11, false) == 0);
    CHECK(delivered == 4);
    CHECK(frame(key, size, 10, true) == 1); // duplicate/late frame
    CHECK(requests == 0 && delivered == 4);

    // A missing reference followed by a delta still needs recovery.
    CHECK(frame(delta, sizeof(delta), 15, false) == 1);
    CHECK(requests == 1 && delivered == 4);
    fakeNow = 999;
    CHECK(frame(delta, sizeof(delta), 16, false) == 1);
    CHECK(requests == 1); // No per-frame request storm.
    fakeNow = 1000;
    CHECK(frame(delta, sizeof(delta), 17, false) == 1);
    CHECK(requests == 2); // Lost response: contiguous deltas must retry.
    CHECK(frame(delta, sizeof(delta), 18, false) == 1);
    CHECK(requests == 2);
    CHECK(frame(key, size, 20, true) == 0);
    CHECK(requests == 2 && delivered == 5);
    CHECK(frame(delta, sizeof(delta), 21, false) == 0);

    // A key flag without an actual random-access picture is not recovery.
    CHECK(frame(key, configSize, 25, true) == -1);
    CHECK(delivered == 6);
    CHECK(frame(delta, sizeof(delta), 26, false) == 1);
    CHECK(frame(key, size, 30, true) == 0);
    CHECK(frame(delta, sizeof(delta), 31, false) == 0);

    // Decoder-requested recovery retains its existing authority.
    decoderResult = DR_NEED_IDR;
    unsigned before = requests;
    CHECK(frame(delta, sizeof(delta), 32, false) == 0);
    CHECK(requests == before + 1);
    decoderResult = DR_OK;
    CHECK(frame(delta, sizeof(delta), 33, false) == 1);
    CHECK(frame(key, size, 40, true) == 0);
    CHECK(requests == before + 1);
    CHECK(frame(delta, sizeof(delta), 41, false) == 0);
    const unsigned char garbage[] = {0xff, 0xff, 0xff, 0xff, 0xff};
    CHECK(frame(garbage, sizeof(garbage), 42, true) == -1);
    CHECK(frame(delta, sizeof(delta), 43, false) == 1);
    CHECK(frame(key, size, 50, true) == 0);
    // A picture without its required parameter sets is not a complete keyframe.
    CHECK(frame(key + configSize, size - configSize, 51, true) == -1);
    CHECK(frame(delta, sizeof(delta), 52, false) == 1);
    CHECK(frame(key, size, 60, true) == 0);
    stopVideoFrameAssembler();
    destroyVideoFrameAssembler();
}

static void queuedRecovery(int codec, const unsigned char* key, size_t size) {
    NegotiatedVideoFormat = codec;
    VideoCallbacks.capabilities = CAPABILITY_PULL_RENDERER;
    pullMode = false; // Deliberately leave real decoder units queued.
    fakeNow = 0;
    requests = 0;
    initializeVideoFrameAssembler();
    CHECK(frame(key, size, 1, true) == 0);
    for (uint32_t n = 2; n <= 15; n++) CHECK(frame(delta, sizeof(delta), n, false) == 0);
    CHECK(LiGetPendingVideoFrames() == 15);
    CHECK(frame(delta, sizeof(delta), 16, false) == 0); // Overflow flushes queue.
    CHECK(LiGetPendingVideoFrames() == 0 && requests == 1);
    fakeNow = 999;
    CHECK(frame(delta, sizeof(delta), 17, false) == 1);
    CHECK(requests == 1);
    fakeNow = 1000;
    CHECK(frame(delta, sizeof(delta), 18, false) == 1);
    CHECK(requests == 2);
    pullMode = true;
    CHECK(frame(key, size, 19, true) == 0);
    CHECK(frame(delta, sizeof(delta), 20, false) == 0);

    // A decoder-thread refresh crosses to the producer at a frame boundary.
    unsigned before = requests;
    requestDecoderRefresh();
    CHECK(requests == before + 1);
    CHECK(frame(delta, sizeof(delta), 21, false) == 1);
    CHECK(frame(key, size, 22, true) == 0);
    CHECK(frame(delta, sizeof(delta), 23, false) == 0);

    // One-Hz input still retries by elapsed time rather than frame count.
    CHECK(frame(delta, sizeof(delta), 25, false) == 1);
    before = requests;
    fakeNow += 1000;
    CHECK(frame(delta, sizeof(delta), 26, false) == 1);
    CHECK(requests == before + 1);
    CHECK(frame(key, size, 27, true) == 0);
    stopVideoFrameAssembler();
    // A producer already in flight at stop cannot transfer ownership to a
    // shutdown queue or trigger another recovery cycle.
    pullMode = false;
    before = requests;
    CHECK(frame(key, size, 28, true) == 0);
    CHECK(LiGetPendingVideoFrames() == 0 && requests == before);
    destroyVideoFrameAssembler();
}

static void* refreshWorker(void* unused) {
    (void)unused;
    for (int n = 0; n < 1000; n++) requestDecoderRefresh();
    return NULL;
}

static void refreshRace(int codec, const unsigned char* key, size_t size) {
    NegotiatedVideoFormat = codec;
    VideoCallbacks.capabilities = CAPABILITY_PULL_RENDERER;
    pullMode = false;
    fakeNow = 0;
    initializeVideoFrameAssembler();
    pthread_t worker;
    CHECK(pthread_create(&worker, NULL, refreshWorker, NULL) == 0);
    for (uint32_t n = 1; n <= 1000; n++) {
        CHECK(frame(key, size, n, true) == 0);
        VIDEO_FRAME_HANDLE handle;
        PDECODE_UNIT unit;
        if (LiPollNextVideoFrame(&handle, &unit)) LiCompleteVideoFrame(handle, DR_OK);
    }
    CHECK(pthread_join(worker, NULL) == 0);
    requestDecoderRefresh(); // Quiescent final generation has a usable keyframe.
    pullMode = true;
    CHECK(frame(key, size, 1001, true) == 0);
    CHECK(frame(delta, sizeof(delta), 1002, false) == 0);
    stopVideoFrameAssembler();
    destroyVideoFrameAssembler();
}

int main(void) {
    ListenerCallbacks.logMessage = quietLog;
    VideoCallbacks.capabilities = CAPABILITY_DIRECT_SUBMIT;
    VideoCallbacks.submitDecodeUnit = submit;
    exercise(VIDEO_FORMAT_H264, h264, sizeof(h264), 12);
    exercise(VIDEO_FORMAT_H265_MAIN10, hevc, sizeof(hevc), 21);
    VideoCallbacks.capabilities = CAPABILITY_PULL_RENDERER;
    pullMode = true;
    exercise(VIDEO_FORMAT_H264, h264, sizeof(h264), 12);
    exercise(VIDEO_FORMAT_H265_MAIN10, hevc, sizeof(hevc), 21);
    queuedRecovery(VIDEO_FORMAT_H264, h264, sizeof(h264));
    queuedRecovery(VIDEO_FORMAT_H265_MAIN10, hevc, sizeof(hevc));
    refreshRace(VIDEO_FORMAT_H264, h264, sizeof(h264));
    refreshRace(VIDEO_FORMAT_H265_MAIN10, hevc, sizeof(hevc));
    printf("native video recovery: %u checks passed\n", checks);
    return 0;
}
