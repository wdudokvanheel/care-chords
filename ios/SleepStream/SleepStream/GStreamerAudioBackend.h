#ifndef GStreamerAudioBackend_h
#define GStreamerAudioBackend_h

#include <stdio.h>
#include <Foundation/Foundation.h>

@interface GStreamerAudioBackend : NSObject

-(id) init:(id) uiDelegate;

-(void) play;

-(void) pause;

-(void) run_app_pipeline_threaded;

@end

#endif /* GStreamerAudioBackend_h */
