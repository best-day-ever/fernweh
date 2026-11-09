// SPDX-License-Identifier: GPL-3.0-or-later
// Acceptance-only decoder: verifies received HEVC pixels without saving them.
#pragma once
#import <VideoToolbox/VideoToolbox.h>

@interface PLANKHEVCDecodeCheck : NSObject
@property(readonly) NSUInteger decodedFrames;
@property(readonly) OSType pixelFormat;
- (BOOL)decode:(const uint8_t *)bytes length:(size_t)length width:(unsigned)width height:(unsigned)height;
@end
@implementation PLANKHEVCDecodeCheck {
    CMVideoFormatDescriptionRef _format;
    VTDecompressionSessionRef _decoder;
    NSData *_parameters[3];
    NSUInteger _decodedFrames;
    OSType _pixelFormat;
}
- (NSUInteger)decodedFrames { return _decodedFrames; }
- (OSType)pixelFormat { return _pixelFormat; }
- (BOOL)decode:(const uint8_t *)bytes length:(size_t)length width:(unsigned)width height:(unsigned)height {
    NSMutableData *sample = [NSMutableData data];
    BOOL image = NO;
    size_t offset = 0;
    while (offset + 4 <= length) {
        if (bytes[offset] || bytes[offset+1] || bytes[offset+2] || bytes[offset+3] != 1) return NO;
        size_t start = offset + 4, end = start;
        while (end + 4 <= length && (bytes[end] || bytes[end+1] || bytes[end+2] || bytes[end+3] != 1)) ++end;
        if (end + 4 > length) end = length;
        if (end - start < 2 || end - start > UINT32_MAX) return NO;
        unsigned type = (bytes[start] >> 1) & 0x3f;
        if (type >= 32 && type <= 34) {
            NSData *parameter = [NSData dataWithBytes:bytes+start length:end-start];
            if (_parameters[type-32] && ![_parameters[type-32] isEqual:parameter]) return NO;
            _parameters[type-32] = parameter;
        } else {
            uint32_t size = CFSwapInt32HostToBig((uint32_t)(end-start));
            [sample appendBytes:&size length:4]; [sample appendBytes:bytes+start length:end-start];
            if (type <= 31) image = YES;
        }
        offset = end;
    }
    if (offset != length || !image || !_parameters[0] || !_parameters[1] || !_parameters[2]) return NO;
    if (!_format) {
        const uint8_t *parameters[] = {_parameters[0].bytes, _parameters[1].bytes, _parameters[2].bytes};
        size_t sizes[] = {_parameters[0].length, _parameters[1].length, _parameters[2].length};
        if (CMVideoFormatDescriptionCreateFromHEVCParameterSets(kCFAllocatorDefault, 3, parameters, sizes,
                4, NULL, &_format)) return NO;
        CMVideoDimensions dimensions = CMVideoFormatDescriptionGetDimensions(_format);
        if (dimensions.width != (int32_t)width || dimensions.height != (int32_t)height ||
            VTDecompressionSessionCreate(kCFAllocatorDefault, _format, NULL, NULL, NULL, &_decoder)) return NO;
    }
    CMBlockBufferRef block = NULL;
    CMSampleBufferRef buffer = NULL;
    if (CMBlockBufferCreateWithMemoryBlock(kCFAllocatorDefault, NULL, sample.length, kCFAllocatorDefault,
            NULL, 0, sample.length, 0, &block)) return NO;
    BOOL ok = !CMBlockBufferReplaceDataBytes(sample.bytes, block, 0, sample.length);
    size_t sampleSize = sample.length;
    if (ok) ok = !CMSampleBufferCreateReady(kCFAllocatorDefault, block, _format, 1, 0, NULL, 1, &sampleSize, &buffer);
    __block BOOL decoded = NO;
    if (ok) ok = !VTDecompressionSessionDecodeFrameWithOutputHandler(_decoder, buffer, 0, NULL,
        ^(OSStatus status, VTDecodeInfoFlags flags, CVImageBufferRef imageBuffer, CMTime pts, CMTime duration) {
            (void)flags; (void)pts; (void)duration;
            if (status || !imageBuffer || CVPixelBufferGetWidth(imageBuffer) != width ||
                CVPixelBufferGetHeight(imageBuffer) != height) return;
            self->_pixelFormat = CVPixelBufferGetPixelFormatType(imageBuffer);
            ++self->_decodedFrames; decoded = YES;
        });
    if (ok) ok = !VTDecompressionSessionWaitForAsynchronousFrames(_decoder);
    if (buffer) CFRelease(buffer);
    CFRelease(block);
    return ok && decoded;
}
- (void)dealloc {
    if (_decoder) { VTDecompressionSessionInvalidate(_decoder); CFRelease(_decoder); }
    if (_format) CFRelease(_format);
}
@end
