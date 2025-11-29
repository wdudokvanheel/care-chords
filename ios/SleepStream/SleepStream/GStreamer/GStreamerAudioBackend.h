#ifndef GStreamerAudioBackend_h
#define GStreamerAudioBackend_h

#include "GStreamerBackend.h"

@interface GStreamerAudioBackend : GStreamerBackend

-(id) init:(id) uiDelegate serverAddress:(NSString *)serverAddress;

@end

#endif 
