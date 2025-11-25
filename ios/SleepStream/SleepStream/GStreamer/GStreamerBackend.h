#ifndef GStreamerBackend_h
#define GStreamerBackend_h

#include <stdio.h>
#include <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import <GStreamer/gst/gst.h>

@protocol GStreamerBackendDelegate;

@interface GStreamerBackend : NSObject

@property (nonatomic, weak) id<GStreamerBackendDelegate> ui_delegate;
@property (nonatomic, assign) GstElement *pipeline;
@property (nonatomic, assign) GMainContext *context;
@property (nonatomic, assign) GMainLoop *main_loop;
@property (nonatomic, assign) gboolean initialized;

-(id) init:(id) uiDelegate;
-(void) play;
-(void) pause;
-(void) stop;
-(void) run_app_pipeline_threaded;
-(void) setUIMessage:(gchar*) message;
-(void) check_initialization_complete;

// Methods to be overridden by subclasses
-(void) run_app_pipeline;
-(void) build_pipeline; 

@end

#endif /* GStreamerBackend_h */
