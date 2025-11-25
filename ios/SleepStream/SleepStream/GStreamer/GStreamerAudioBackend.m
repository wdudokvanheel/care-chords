#import "GStreamerAudioBackend.h"
#import "gst_ios_init.h"
#import <UIKit/UIKit.h>
#import <AVFoundation/AVFoundation.h>

#import <GStreamer/gst/gst.h>
#import <GStreamer/gst/rtsp/rtsp.h>
#import "SleepStream-Bridging-Header.h"
#import "Care_Chords-Swift.h"

@interface GStreamerAudioBackend()
@end

@implementation GStreamerAudioBackend {
    /* New elements */
    GstElement *rtspsrc;
    GstElement *depayloader;
    GstElement *queue;
    GstElement *parser;
    GstElement *decoder;
    GstElement *converter;
    GstElement *sampler;
    GstElement *audio_sink;
}

/*
 * Interface methods
 */

-(id) init:(id) uiDelegate
{
    if (self = [super init:uiDelegate]) {
        [[NSNotificationCenter defaultCenter] addObserver:self
                                                 selector:@selector(handleInterruption:)
                                                     name:AVAudioSessionInterruptionNotification
                                                   object:nil];
    }
    return self;
}

-(void) dealloc {
    [[NSNotificationCenter defaultCenter] removeObserver:self];
}

-(void) handleInterruption:(NSNotification *)notification {
    NSDictionary *interruptionDict = notification.userInfo;
    NSNumber *interruptionType = [interruptionDict valueForKey:AVAudioSessionInterruptionTypeKey];
    
    if ([interruptionType unsignedIntegerValue] == AVAudioSessionInterruptionTypeBegan) {
        printf("Audio interruption began\n");
        [self pause];
    } else if ([interruptionType unsignedIntegerValue] == AVAudioSessionInterruptionTypeEnded) {
        NSNumber *interruptionOption = [interruptionDict valueForKey:AVAudioSessionInterruptionOptionKey];
        if ([interruptionOption unsignedIntegerValue] == AVAudioSessionInterruptionOptionShouldResume) {
            printf("Audio interruption ended, resuming\n");
            [self play];
        }
    }
}

-(void) stateChanged:(GstState)newState old:(GstState)oldState pending:(GstState)pendingState {
    switch (newState) {
        case GST_STATE_PLAYING:
            if (self.ui_delegate) {
                dispatch_async(dispatch_get_main_queue(), ^{
                    [(id<GStreamerAudioBackendDelegate>)self.ui_delegate gstreamerAudioStateWithState:AudioStatePlaying];
                });
            }
            break;

        case GST_STATE_PAUSED:
            if (self.ui_delegate) {
                dispatch_async(dispatch_get_main_queue(), ^{
                    [(id<GStreamerAudioBackendDelegate>)self.ui_delegate gstreamerAudioStateWithState:AudioStatePaused];
                });
            }
            break;

        case GST_STATE_READY:
            // Auto state playback when ready
            [self play];
            if (self.ui_delegate) {
                dispatch_async(dispatch_get_main_queue(), ^{
                    [(id<GStreamerAudioBackendDelegate>)self.ui_delegate gstreamerAudioStateWithState:AudioStateReady];
                });
            }
            break;

        case GST_STATE_NULL:
            if (self.ui_delegate) {
                dispatch_async(dispatch_get_main_queue(), ^{
                    [(id<GStreamerAudioBackendDelegate>)self.ui_delegate gstreamerAudioStateWithState:AudioStateStopped];
                });
            }
            break;

        default:
            break;
    }
}

static void on_pad_added(GstElement *src, GstPad *new_pad, GStreamerAudioBackend *self)
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

    printf("Received new pad '%s' from '%s':\n", new_pad_type, GST_ELEMENT_NAME(src));

    if (g_str_has_prefix(new_pad_type, "application/x-rtp")) {
        /* Check if it's audio */
        const gchar *media = gst_structure_get_string(str, "media");
        if (g_strcmp0(media, "audio") == 0) {
            GstPad *sink_pad = gst_element_get_static_pad(self->depayloader, "sink");
            GstPadLinkReturn ret;

            /* Attempt to link the dynamic pad to depayloader sink pad */
            ret = gst_pad_link(new_pad, sink_pad);
            if (GST_PAD_LINK_FAILED(ret)) {
                gchar *message = g_strdup_printf("Failed to link dynamic audio pad.");
                [self setUIMessage:message];
                g_free(message);
            } else {
                GST_DEBUG("Link succeeded (audio).");
            }
            gst_object_unref(sink_pad);
        } else if (g_strcmp0(media, "video") == 0) {
            printf("Ignoring video pad.\n");
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
    self->depayloader = gst_element_factory_make("rtpmp4adepay", "depay");
    self->queue = gst_element_factory_make("queue", "queue");
    self->parser = gst_element_factory_make("aacparse", "parser");
    self->decoder = gst_element_factory_make("avdec_aac", "decoder");
    self->converter = gst_element_factory_make("audioconvert", "converter");
    self->sampler = gst_element_factory_make("audioresample", "sampler");
    self->audio_sink = gst_element_factory_make("autoaudiosink", "audiosink");

    if (!self.pipeline || !self->rtspsrc || !self->depayloader || !self->queue || !self->parser || !self->decoder || !self->audio_sink || !self->converter || !self->sampler) {
        gchar *message = g_strdup_printf("Not all elements could be created.");
        [self setUIMessage:message];
        g_free(message);
        self.pipeline = NULL; // Signal failure
        return;
    }

    /* Set element properties */
    g_object_set(self->rtspsrc, "location", "rtsp://10.0.0.12:7554/sleep", NULL);
    g_object_set(self->rtspsrc, "protocols", GST_RTSP_LOWER_TRANS_TCP, NULL);

    /* Add elements to the pipeline */
    gst_bin_add_many(GST_BIN(self.pipeline), self->rtspsrc, self->depayloader, self->queue, self->parser, self->decoder, self->converter, self->sampler, self->audio_sink, NULL);

    /* Link the elements (except rtspsrc, which is linked dynamically) */
    if (!gst_element_link_many(self->depayloader, self->queue, self->parser, self->decoder, self->converter, self->sampler, self->audio_sink, NULL)) {
        gchar *message = g_strdup_printf("Elements could not be linked.");
        [self setUIMessage:message];
        g_free(message);
        gst_object_unref(self.pipeline);
        self.pipeline = NULL; // Signal failure
        return;
    }

    /* Connect to the pad-added signal for dynamic pad linking */
    g_signal_connect(self->rtspsrc, "pad-added", G_CALLBACK(on_pad_added), (__bridge void *)self);

    /* Set the pipeline to READY */
    gst_element_set_state(self.pipeline, GST_STATE_READY);
}

-(void) stop {
    if (self.ui_delegate) {
        [(id<GStreamerAudioBackendDelegate>)self.ui_delegate gstreamerAudioStateWithState:AudioStateStopped];
    }
    [super stop];
}

// Override pause to include seek logic if needed, or keep base implementation if seek is not strictly required for pause
// The original implementation had a seek.
-(void) pause
{
    printf("Pausing playback\n");
    if(gst_element_set_state(self.pipeline, GST_STATE_PAUSED) == GST_STATE_CHANGE_FAILURE) {
        [self setUIMessage:"Failed to set pipeline to paused"];
    }
    else{
        gst_element_seek(self.pipeline, 1.0, GST_FORMAT_TIME, GST_SEEK_FLAG_FLUSH, GST_SEEK_TYPE_SET, 0, GST_SEEK_TYPE_NONE, GST_CLOCK_TIME_NONE);
    }
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
