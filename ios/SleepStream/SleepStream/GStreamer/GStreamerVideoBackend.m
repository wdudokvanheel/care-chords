#import "GStreamerVideoBackend.h"
#import "gst_ios_init.h"
#import <UIKit/UIKit.h>
#import <CoreMedia/CoreMedia.h>
#import <VideoToolbox/VideoToolbox.h>

#import <GStreamer/gst/gst.h>
#import <GStreamer/gst/video/video.h>
#import <GStreamer/gst/rtsp/rtsp.h>
#import <GStreamer/gst/app/gstappsink.h>
#import "SleepStream-Bridging-Header.h"
#import "Care_Chords-Swift.h"

@interface GStreamerVideoBackend()
@end

@implementation GStreamerVideoBackend {
    UIView *ui_video_view;     /* UIView that holds the video */
    
    /* New elements */
    GstElement *rtspsrc;
    GstElement *rtph264depay;
    GstElement *queue;
    GstElement *h264parse;
    GstElement *avdec_h264;
    GstElement *videocrop;
    GstElement *appsink;
    GstElement *capsfilter;
    GstElement *videoconvert;
    GstElement *videoscale;
}

/*
 * Interface methods
 */

-(id) init:(id) uiDelegate videoView:(UIView *)video_view
{
    if (self = [super init:uiDelegate])
    {
        self->ui_video_view = video_view;
    }
    return self;
}

-(void)setWindow:(UIView *)video_view
{
    self->ui_video_view = video_view;
}

-(void) stateChanged:(GstState)newState old:(GstState)oldState pending:(GstState)pendingState {
    if (newState == GST_STATE_READY){
        [self play];
    }
    
    // Original implementation sent a generic message on state change
    gchar *message = g_strdup_printf("State changed from %s to %s", gst_element_state_get_name(oldState), gst_element_state_get_name(newState));
    [self setUIMessage:message];
    g_free (message);
}

static void cb_new_decoded_caps(GObject *padObject, GParamSpec *pspec, gpointer user_data)
{
    GStreamerVideoBackend *self = (__bridge GStreamerVideoBackend *)user_data;
    GstPad *pad = GST_PAD(padObject);

    // Retrieve the current caps from this pad
    GstCaps *caps = gst_pad_get_current_caps(pad);
    if (!caps) return;

    // Extract width/height from the caps
    const GstStructure *s = gst_caps_get_structure(caps, 0);
    gint width = 0, height = 0;
    gboolean hasWidth = gst_structure_get_int(s, "width", &width);
    gboolean hasHeight = gst_structure_get_int(s, "height", &height);

    if (hasWidth && hasHeight) {
        // Dispatch to the main thread and call the new delegate method
        dispatch_async(dispatch_get_main_queue(), ^{
            [(id<GStreamerVideoBackendDelegate>)self.ui_delegate gstreamerDidReceiveVideoResolutionWithWidth:width
                                                                   height:height];
        });
    }

    gst_caps_unref(caps);
}

static GstFlowReturn on_new_sample(GstElement *sink, gpointer user_data) {
    GStreamerVideoBackend *self = (__bridge GStreamerVideoBackend *)user_data;
    GstSample *sample = gst_app_sink_pull_sample(GST_APP_SINK(sink));
    
    if (sample) {
        GstCaps *caps = gst_sample_get_caps(sample);
        GstVideoInfo info;
        
        if (gst_video_info_from_caps(&info, caps)) {
            GstBuffer *buffer = gst_sample_get_buffer(sample);
            GstMapInfo map;
            
            if (gst_buffer_map(buffer, &map, GST_MAP_READ)) {
                int width = info.width;
                int height = info.height;
                
                CVPixelBufferRef pixelBuffer = NULL;
                NSDictionary *options = @{
                    (__bridge NSString *)kCVPixelBufferCGImageCompatibilityKey: @YES,
                    (__bridge NSString *)kCVPixelBufferCGBitmapContextCompatibilityKey: @YES,
                    (__bridge NSString *)kCVPixelBufferIOSurfacePropertiesKey: @{}
                };
                
                CVReturn status = CVPixelBufferCreate(kCFAllocatorDefault, width, height, kCVPixelFormatType_32BGRA, (__bridge CFDictionaryRef)options, &pixelBuffer);
                
                if (status == kCVReturnSuccess && pixelBuffer != NULL) {
                    CVPixelBufferLockBaseAddress(pixelBuffer, 0);
                    void *pxdata = CVPixelBufferGetBaseAddress(pixelBuffer);
                    size_t bytesPerRow = CVPixelBufferGetBytesPerRow(pixelBuffer);
                    
                    // Copy line by line to handle stride mismatch
                    // GStreamer BGRA is usually packed, but let's be safe
                    // info.stride[0] is the GStreamer stride
                    int gst_stride = info.stride[0];
                    int copy_width = width * 4; // 4 bytes per pixel for BGRA
                    
                    for (int i = 0; i < height; i++) {
                        memcpy(pxdata + i * bytesPerRow, map.data + i * gst_stride, copy_width);
                    }
                    
                    CVPixelBufferUnlockBaseAddress(pixelBuffer, 0);
                    
                    // Create CMSampleBuffer
                    CMSampleBufferRef sampleBuffer = NULL;
                    CMVideoFormatDescriptionRef videoInfo = NULL;
                    CMVideoFormatDescriptionCreateForImageBuffer(NULL, pixelBuffer, &videoInfo);
                    
                    CMSampleTimingInfo timingInfo;
                    timingInfo.duration = kCMTimeInvalid;
                    timingInfo.decodeTimeStamp = kCMTimeInvalid;
                    timingInfo.presentationTimeStamp = CMTimeMake(GST_BUFFER_PTS(buffer), GST_SECOND);
                    
                    CMSampleBufferCreateForImageBuffer(kCFAllocatorDefault, pixelBuffer, true, NULL, NULL, videoInfo, &timingInfo, &sampleBuffer);
                    
                    if (sampleBuffer) {
                        dispatch_async(dispatch_get_main_queue(), ^{
                            if (self.ui_delegate) {
                                [(id<GStreamerVideoBackendDelegate>)self.ui_delegate gstreamerDidReceiveSampleBuffer:sampleBuffer];
                            }
                            CFRelease(sampleBuffer);
                        });
                    }
                    
                    if (videoInfo) CFRelease(videoInfo);
                    CVPixelBufferRelease(pixelBuffer);
                }
                gst_buffer_unmap(buffer, &map);
            }
        }
        gst_sample_unref(sample);
        return GST_FLOW_OK;
    }
    return GST_FLOW_ERROR;
}

static void on_pad_added(GstElement *src, GstPad *new_pad, GStreamerVideoBackend *self)
{
    GstCaps *caps;
    GstStructure *str;
    const gchar *new_pad_type;

    /* Check the new pad's type */
    caps = gst_pad_get_current_caps(new_pad);
    if (!caps) {
        caps = gst_pad_query_caps(new_pad, NULL);
    }
    str = gst_caps_get_structure(caps, 0);
    new_pad_type = gst_structure_get_name(str);
    
    if (!caps) {
        printf("No caps available for the new pad.\n");
        gst_caps_unref(caps);
        return;
    }
    
    // In on_pad_added, print the caps for debugging
    GstCaps *caps2 = gst_pad_get_current_caps(new_pad);
    gchar *caps_str = gst_caps_to_string(caps2);
    g_free(caps_str);
    gst_caps_unref(caps2);
    
    GstPad *decoder_src_pad = gst_element_get_static_pad(self->avdec_h264, "src");
    if (decoder_src_pad) {
        g_signal_connect(decoder_src_pad, "notify::caps", G_CALLBACK(cb_new_decoded_caps), (__bridge void *)self);
        gst_object_unref(decoder_src_pad);
    }

    if (g_str_has_prefix(new_pad_type, "application/x-rtp")) {
        /* Check if it's video */
        const gchar *media = gst_structure_get_string(str, "media");
        if (g_strcmp0(media, "video") == 0) {
            GstPad *sink_pad = gst_element_get_static_pad(self->rtph264depay, "sink");
            GstPadLinkReturn ret;

            /* Attempt to link the dynamic pad to rtph264depay sink pad */
            ret = gst_pad_link(new_pad, sink_pad);
            if (GST_PAD_LINK_FAILED(ret)) {
                gchar *message = g_strdup_printf("Failed to link dynamic video pad.");
                [self setUIMessage:message];
                g_free(message);
            } else {
                GST_DEBUG("Link succeeded (video).");
            }
            gst_object_unref(sink_pad);
        } else if (g_strcmp0(media, "audio") == 0) {
            printf("Ignoring audio pad.\n");
        }
    } else {
        printf("Unknown pad type: %s\n", new_pad_type);
    }

    gst_caps_unref(caps);
}

-(void) build_pipeline
{
    /* Create the pipeline and elements */
    self.pipeline = gst_pipeline_new("pipeline");

    self->rtspsrc = gst_element_factory_make("rtspsrc", "source");
    self->rtph264depay = gst_element_factory_make("rtph264depay", "depay");
    self->queue = gst_element_factory_make("queue", "queue");
    self->h264parse = gst_element_factory_make("h264parse", "parse");
    self->avdec_h264 = gst_element_factory_make("avdec_h264", "decoder");
    self->videocrop      = gst_element_factory_make("videocrop",     "videocrop");
    self->appsink = gst_element_factory_make("appsink", "videosink");
    self->videoconvert = gst_element_factory_make("videoconvert", "videoconvert");
    self->videoscale = gst_element_factory_make("videoscale", "videoscale");
    
    // Configure appsink
    g_object_set(self->appsink, "emit-signals", TRUE, "sync", FALSE, NULL);
    g_signal_connect(self->appsink, "new-sample", G_CALLBACK(on_new_sample), (__bridge void *)self);

    int totalHeight = 1920;
    int totalWidth = 2560;
    
    g_object_set(self->rtspsrc,
                 "location", "rtsp://sleepstream:sleepstream@10.0.0.51",
                 "protocols", GST_RTSP_LOWER_TRANS_TCP,
                 "latency", 0,
                 "buffermode", 0,
                 NULL);
    
    // Create a capsfilter with desired caps
    // We force BGRA for compatibility with CVPixelBuffer creation
    GstCaps *caps = gst_caps_new_simple("video/x-raw",
                                        "format", G_TYPE_STRING, "BGRA",
                                        "width", G_TYPE_INT, totalWidth,
                                        "height", G_TYPE_INT, totalHeight,
                                        NULL);

    int cropWidth = totalWidth * 0.4;
    int cropHeight = totalHeight * 0.4;
    
    int top = 352;
    int left = 1024;
    int bottom = totalHeight - cropHeight - top;
    int right = totalWidth - cropWidth - left;
    
    g_object_set(self->videocrop,
                    "top",    top,
                    "left",   left,
                    "bottom", bottom,
                    "right",  right,
                    NULL);
    
    self->capsfilter = gst_element_factory_make("capsfilter", "capsfilter");
    g_object_set(self->capsfilter, "caps", caps, NULL);
    gst_caps_unref(caps);

    if (!self.pipeline || !self->rtspsrc || !self->rtph264depay || !self->queue || !self->h264parse || !self->avdec_h264 || !self->videocrop  || !self->videoscale || !self->videoconvert|| !self->appsink || !self->capsfilter) {
        gchar *message = g_strdup_printf("Not all elements could be created.");
        [self setUIMessage:message];
        g_free(message);
        self.pipeline = NULL;
        return;
    }

    /* Add elements to the pipeline */
    gst_bin_add_many(GST_BIN(self.pipeline), self->rtspsrc, self->rtph264depay, self->queue, self->h264parse, self->avdec_h264, self->videocrop, self->videoconvert,
                     self->videoscale,  self->capsfilter, self->appsink, NULL);

    /* Link the elements (except rtspsrc, which is linked dynamically) */
    if (!gst_element_link_many(self->rtph264depay, self->queue, self->h264parse, self->avdec_h264, self->videocrop, self->videoconvert, self->videoscale, self->capsfilter,  self->appsink, NULL)) {
        gchar *message = g_strdup_printf("Elements could not be linked.");
        [self setUIMessage:message];
        g_free(message);
        gst_object_unref(self.pipeline);
        self.pipeline = NULL;
        return;
    }

    /* Connect to the pad-added signal for dynamic pad linking */
    g_signal_connect(self->rtspsrc, "pad-added", G_CALLBACK(on_pad_added), (__bridge void *)self);

    /* Set the pipeline to READY */
    gst_element_set_state(self.pipeline, GST_STATE_READY);
}

-(void) play
{
    if (gst_element_set_state(self.pipeline, GST_STATE_PLAYING) == GST_STATE_CHANGE_FAILURE) {
        [self setUIMessage:"Failed to set pipeline to playing"];
        return;
    }
}

-(void) stopAndCleanup {
    // Safely remove subviews on the main thread
    dispatch_async(dispatch_get_main_queue(), ^{
        if (self->ui_video_view) {
            NSArray *subviews = [self->ui_video_view subviews];
            for (UIView *subview in subviews) {
                [subview removeFromSuperview];
            }
        }
    });

    [self stop];
}

-(void) destroy
{
    if(gst_element_set_state(self.pipeline, GST_STATE_PAUSED) == GST_STATE_CHANGE_FAILURE) {
        [self setUIMessage:"Failed to set pipeline to READY"];
    }
    GstMessage* eos_msg = gst_message_new_eos(GST_OBJECT(self.pipeline));
    gst_element_post_message (self.pipeline, eos_msg);
}

@end
