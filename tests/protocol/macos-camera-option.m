// SPDX-License-Identifier: GPL-3.0-or-later
#import "media-features.h"
#include <assert.h>
#include <stdio.h>
int main(int argc, char **argv) {
    @autoreleasepool {
        assert(argc == 2);
        NSData *data = [NSData dataWithContentsOfFile:@(argv[1])];
        NSDictionary *fixture = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
        NSMutableDictionary *offer = [fixture[@"offer"] mutableCopy];
        assert(offer);
        unsigned status = 0;
        NSDictionary *reply = PLANKMacNegotiateMedia(offer, &status);
        assert(reply && status == 200);
        if (PLANK_MACOS_CAMERA_EXTENSION) {
            assert([reply[@"features"][@"camera"] isKindOfClass:NSDictionary.class]);
        } else {
            assert(reply[@"features"][@"camera"] == NSNull.null);
            for (NSString *name in @[@"desktop", @"audio", @"input", @"microphone"])
                assert([reply[@"features"][name] isKindOfClass:NSDictionary.class]);
        }
        offer[@"required_features"] = [offer[@"required_features"] arrayByAddingObject:@"camera"];
        reply = PLANKMacNegotiateMedia(offer, &status);
        assert(PLANK_MACOS_CAMERA_EXTENSION ? (reply && status == 200) : (!reply && status == 426));
        printf("camera variant=%d: optional/required negotiation passed\n", PLANK_MACOS_CAMERA_EXTENSION);
    }
}
