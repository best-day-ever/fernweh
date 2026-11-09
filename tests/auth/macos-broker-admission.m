// SPDX-License-Identifier: GPL-3.0-or-later
#import <Foundation/Foundation.h>
#import <Security/Security.h>
#import <CommonCrypto/CommonDigest.h>
#import "broker-admission.h"
#include <pwd.h>
#include <unistd.h>
#include <sys/stat.h>
#include <stdatomic.h>

#define CHECK(condition) do { if (!(condition)) { fprintf(stderr, "broker admission failure line %d\n", __LINE__); return 1; } } while (0)
static NSString *url(NSData *bytes) {
    return [[[[bytes base64EncodedStringWithOptions:0] stringByReplacingOccurrencesOfString:@"+" withString:@"-"]
        stringByReplacingOccurrencesOfString:@"/" withString:@"_"] stringByReplacingOccurrencesOfString:@"=" withString:@""];
}
static NSString *sign(SecKeyRef key, NSDictionary *claims) {
    NSData *json = [NSJSONSerialization dataWithJSONObject:claims options:NSJSONWritingSortedKeys error:NULL];
    NSString *message = [@"v1." stringByAppendingString:url(json)];
    NSData *signature = CFBridgingRelease(SecKeyCreateSignature(key, kSecKeyAlgorithmECDSASignatureMessageX962SHA256,
        (__bridge CFDataRef)[message dataUsingEncoding:NSASCIIStringEncoding], NULL));
    return [message stringByAppendingFormat:@".%@", url(signature)];
}
int main(void) { @autoreleasepool {
    SecKeyRef key = SecKeyCreateRandomKey((__bridge CFDictionaryRef)@{
        (__bridge id)kSecAttrKeyType:(__bridge id)kSecAttrKeyTypeECSECPrimeRandom,
        (__bridge id)kSecAttrKeySizeInBits:@256}, NULL);
    CHECK(key);
    SecKeyRef publicKey = SecKeyCopyPublicKey(key);
    NSData *public = CFBridgingRelease(SecKeyCopyExternalRepresentation(publicKey, NULL));
    CFRelease(publicKey);
    NSData *certificate = [@"synthetic pinned leaf" dataUsingEncoding:NSUTF8StringEncoding];
    unsigned char hash[CC_SHA256_DIGEST_LENGTH];
    CC_SHA256(certificate.bytes, (CC_LONG)certificate.length, hash);
    NSMutableString *digest = [NSMutableString new];
    for (NSUInteger i = 0; i < sizeof(hash); ++i) [digest appendFormat:@"%02x", hash[i]];
    NSString *local = [NSString stringWithUTF8String:getpwuid(geteuid())->pw_name];
    NSString *directory = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    CHECK([[NSFileManager defaultManager] createDirectoryAtPath:directory withIntermediateDirectories:NO
        attributes:@{NSFilePosixPermissions:@0700} error:NULL]);
    NSString *path = [directory stringByAppendingPathComponent:@"nonces"];
    PLANKMacBrokerAdmission *(^verifier)(void) = ^{
        return [[PLANKMacBrokerAdmission alloc] initWithPublicKey:public host:@"ws.example.org"
            localUser:local certificateDER:certificate replayPath:path];
    };
    PLANKMacBrokerAdmission *admission = verifier();
    CHECK(admission);
    NSMutableDictionary *claims = [@{@"v":@1, @"host":@"ws.example.org", @"cert_sha256":digest,
        @"subject":@"artist", @"local_user":local, @"iat":@1000, @"exp":@1060,
        @"nonce":[@"a" stringByPaddingToLength:64 withString:@"a" startingAtIndex:0]} mutableCopy];
    PLANKMacAccountIdentity identity;
    // All policy rejection paths leave the nonce available for a valid retry.
    CHECK(![admission verifyToken:sign(key, claims) subject:@"different" now:1000 identity:&identity]);
    CHECK(![admission verifyToken:sign(key, claims) subject:@"artist" now:990 identity:&identity]);
    CHECK(![admission verifyToken:sign(key, claims) subject:@"artist" now:1066 identity:&identity]);
    for (NSString *field in @[@"host", @"cert_sha256", @"local_user", @"subject", @"nonce"]) {
        NSMutableDictionary *invalid = [claims mutableCopy]; invalid[field] = @"invalid";
        CHECK(![admission verifyToken:sign(key, invalid) subject:@"artist" now:1000 identity:&identity]);
    }
    for (NSDictionary *change in @[@{@"v":@2}, @{@"iat":@YES}, @{@"exp":@1061}, @{@"exp":@999}, @{@"extra":@1}]) {
        NSMutableDictionary *invalid = [claims mutableCopy]; [invalid addEntriesFromDictionary:change];
        CHECK(![admission verifyToken:sign(key, invalid) subject:@"artist" now:1000 identity:&identity]);
    }
    NSString *token = sign(key, claims);
    NSString *tampered = [token stringByReplacingCharactersInRange:NSMakeRange(token.length - 5, 1) withString:@"_"];
    CHECK(![admission verifyToken:tampered subject:@"artist" now:1000 identity:&identity]);
    CHECK([admission verifyToken:token subject:@"artist" now:1000 identity:&identity]);
    CHECK(identity.uid == geteuid());
    CHECK(![admission verifyToken:token subject:@"artist" now:1000 identity:&identity]);
    CHECK(identity.uid == 0);
    admission = verifier(); // New object/process uses the same durable replay journal.
    CHECK(![admission verifyToken:token subject:@"artist" now:1000 identity:&identity]);
    claims[@"nonce"] = [@"b" stringByPaddingToLength:64 withString:@"b" startingAtIndex:0];
    token = sign(key, claims);
    __block atomic_uint successes; atomic_init(&successes, 0);
    dispatch_apply(8, dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^(size_t iteration) {
        (void)iteration;
        PLANKMacAccountIdentity candidate;
        if ([verifier() verifyToken:token subject:@"artist" now:1000 identity:&candidate]) atomic_fetch_add(&successes, 1);
    });
    CHECK(atomic_load(&successes) == 1);
    CHECK(![admission verifyToken:sign(key, claims) subject:@"artist" now:1000 identity:&identity]);
    claims[@"nonce"] = [@"c" stringByPaddingToLength:64 withString:@"c" startingAtIndex:0];
    CHECK(chmod(path.fileSystemRepresentation, 0644) == 0);
    CHECK(![admission verifyToken:sign(key, claims) subject:@"artist" now:1000 identity:&identity]);
    CHECK(chmod(path.fileSystemRepresentation, 0600) == 0);
    CHECK([[NSFileManager defaultManager] removeItemAtPath:path error:NULL]);
    CHECK(symlink("/dev/null", path.fileSystemRepresentation) == 0);
    CHECK(![admission verifyToken:sign(key, claims) subject:@"artist" now:1000 identity:&identity]);
    CHECK([[NSFileManager defaultManager] removeItemAtPath:path error:NULL]);
    CHECK([@"partial record" writeToFile:path atomically:NO encoding:NSUTF8StringEncoding error:NULL]);
    CHECK(chmod(path.fileSystemRepresentation, 0600) == 0);
    CHECK(![admission verifyToken:sign(key, claims) subject:@"artist" now:1000 identity:&identity]);
    CHECK([[NSFileManager defaultManager] removeItemAtPath:directory error:NULL]);
    CFRelease(key);
    puts("macos_broker_admission=pass signature audience identity expiry replay restart concurrency journal");
    return 0;
} }
