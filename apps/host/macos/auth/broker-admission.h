// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#import <Foundation/Foundation.h>
#import "account-policy.h"

// Explicit administrator opt-in for an unjoined shared desktop. The broker is
// an authentication authority in this mode. No local password is transported.
// The caller must still bind the result to the current graphical generation.
@interface PLANKMacBrokerAdmission : NSObject
- (instancetype)initWithPublicKey:(NSData *)publicKey host:(NSString *)host
                       localUser:(NSString *)localUser certificateDER:(NSData *)certificateDER
                     replayPath:(NSString *)replayPath;
// Consumes the signed nonce durably before returning a local OS identity.
// The supplied time enables bounded deterministic qualification; production
// callers supply time(NULL). Never log the token or the returned claims.
- (BOOL)verifyToken:(NSString *)token subject:(NSString *)subject now:(uint64_t)now
           identity:(PLANKMacAccountIdentity *)identity;
@end
