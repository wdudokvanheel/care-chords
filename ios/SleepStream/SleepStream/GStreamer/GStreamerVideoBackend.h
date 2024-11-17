#ifndef GStreamerBackend_h
#define GStreamerBackend_h

#include <stdio.h>
#include <Foundation/Foundation.h>
#import <UIKit/UIKit.h>

@interface GStreamerVideoBackend : NSObject

-(id) init:(id) uiDelegate videoView:(UIView*) video_view;

-(void) play;

-(void) pause;

-(void) run_app_pipeline_threaded;

-(void)stopAndCleanup;

-(void)setWindow:(UIView *)video_view;

@end

#endif
