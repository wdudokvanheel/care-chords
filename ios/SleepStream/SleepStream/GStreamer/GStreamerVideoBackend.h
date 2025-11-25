#ifndef GStreamerVideoBackend_h
#define GStreamerVideoBackend_h

#include "GStreamerBackend.h"

@interface GStreamerVideoBackend : GStreamerBackend

-(id) init:(id) uiDelegate videoView:(UIView*) video_view;
-(void) setWindow:(UIView *)video_view;
-(void) stopAndCleanup;

@end

#endif
