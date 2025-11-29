#ifndef GStreamerVideoBackend_h
#define GStreamerVideoBackend_h

#include "GStreamerBackend.h"

@interface GStreamerVideoBackend : GStreamerBackend

-(id) init:(id) uiDelegate videoView:(UIView*) video_view;
-(void) setWindow:(UIView *)video_view;
-(void) setMonitorUrl:(NSString *)url;
-(void) stopAndCleanup;

@end

#endif
