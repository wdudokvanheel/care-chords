#import <unistd.h>
#import "GStreamerVideoBackend.h"
#import "gst_ios_init.h"
#import <UIKit/UIKit.h>

#import <GStreamer/gst/gst.h>
#import <GStreamer/gst/video/video.h>
#import <GStreamer/gst/rtsp/rtsp.h>
#import "SleepStream-Bridging-Header.h"

GST_DEBUG_CATEGORY_STATIC (debug_category);
#define GST_CAT_DEFAULT debug_category

#import "Care_Chords-Swift.h"

@interface GStreamerVideoBackend()
-(void)setUIMessage:(gchar*) message;
-(void)run_app_pipeline;
-(void)check_initialization_complete;
@end

@implementation GStreamerVideoBackend {
    id<GStreamerBackendDelegate> ui_delegate;        /* Class that we use to interact with the user interface */
    GstElement *pipeline;      /* The running pipeline */
    GstElement *video_sink;    /* The video sink element which receives XOverlay commands */
    GMainContext *context;     /* GLib context used to run the main loop */
    GMainLoop *main_loop;      /* GLib main loop */
    gboolean initialized;      /* To avoid informing the UI multiple times about the initialization */
    GstBus *bus;
    UIView *ui_video_view;     /* UIView that holds the video */
    GstMessage* eos_msg;

    /* New elements */
    GstElement *rtspsrc;
    GstElement *rtph264depay;
    GstElement *queue;
    GstElement *h264parse;
    GstElement *avdec_h264;
    GstElement *autovideosink;
}

/*
 * Interface methods
 */

-(id) init:(id) uiDelegate videoView:(UIView *)video_view
{
    if (self = [super init])
    {
        self->ui_delegate = (id<GStreamerBackendDelegate>)uiDelegate;
        self->ui_video_view = video_view;

        GST_DEBUG_CATEGORY_INIT (debug_category, "SleepStreamer", 0, "SleepStreamer-Backend");
        gst_debug_set_threshold_for_name("SleepStreamer", GST_LEVEL_TRACE);
    }

    return self;
}

-(void) run_app_pipeline_threaded
{
    [self run_app_pipeline];
    return;
}

-(void) play
{
    if(gst_element_set_state(pipeline, GST_STATE_PLAYING) == GST_STATE_CHANGE_FAILURE) {
        [self setUIMessage:"Failed to set pipeline to playing"];
    }
}

-(void) pause
{
    if(gst_element_set_state(pipeline, GST_STATE_PAUSED) == GST_STATE_CHANGE_FAILURE) {
        [self setUIMessage:"Failed to set pipeline to paused"];
    }
}

-(void) destroy
{
    if(gst_element_set_state(pipeline, GST_STATE_PAUSED) == GST_STATE_CHANGE_FAILURE) {
        [self setUIMessage:"Failed to set pipeline to READY"];
    }
    eos_msg = gst_message_new_eos(GST_OBJECT(pipeline));
    gst_element_post_message (pipeline, eos_msg);
}

/* Change the message on the UI through the UI delegate */
-(void)setUIMessage:(gchar*) message
{
    printf("Setting message to: %s\n", message);
    NSString *messagString = [NSString stringWithUTF8String:message];
    if(ui_delegate)
    {
        [ui_delegate gstreamerMessageWithMessage:messagString];
    }
}

static void eos_cb(GstBus *bus, GstMessage *msg, GStreamerVideoBackend *self){
    printf("\nEOS called\n");
    gst_element_set_state (self->pipeline, GST_STATE_NULL);
    g_main_loop_quit(self->main_loop);
}

/* Retrieve errors from the bus and show them on the UI */
static void error_cb (GstBus *bus, GstMessage *msg, GStreamerVideoBackend *self)
{
    GError *err;
    gchar *debug_info;
    gchar *message_string;

    gst_message_parse_error (msg, &err, &debug_info);
    message_string = g_strdup_printf ("Error received from element %s: %s", GST_OBJECT_NAME (msg->src), err->message);
    printf("Error from element %s: %s\n", GST_OBJECT_NAME (msg->src), err->message);
    g_clear_error (&err);
    g_free (debug_info);
    [self setUIMessage:message_string];
    g_free (message_string);
    gst_element_set_state (self->pipeline, GST_STATE_NULL);
}

/* Notify UI about pipeline state changes */
static void state_changed_cb (GstBus *bus, GstMessage *msg, GStreamerVideoBackend *self)
{
    GstState old_state, new_state, pending_state;
    gst_message_parse_state_changed (msg, &old_state, &new_state, &pending_state);

    /* Only pay attention to messages coming from the pipeline, not its children */
    if (GST_MESSAGE_SRC (msg) == GST_OBJECT (self->pipeline)) {
        printf("State changed from %s to %s\n", gst_element_state_get_name(old_state), gst_element_state_get_name(new_state));
        
        if (new_state == GST_STATE_READY) {
            [self play];
        }

        gchar *message = g_strdup_printf("State changed from %s to %s", gst_element_state_get_name(old_state), gst_element_state_get_name(new_state));
        [self setUIMessage:message];
        g_free (message);
    }
}

/* Check if all conditions are met to report GStreamer as initialized.
 * These conditions will change depending on the application */
-(void) check_initialization_complete
{
    if (!initialized && main_loop) {
        GST_DEBUG ("Initialization complete, notifying application.");
        if (ui_delegate)
        {
            [ui_delegate gStreamerInitialized];
        }
        initialized = TRUE;
    }
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

    printf("Received new pad '%s' from '%s':\n", new_pad_type, GST_ELEMENT_NAME(src));

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

/* Main method */
-(void) run_app_pipeline
{
    GSource *bus_source;
    GST_DEBUG ("Creating pipeline");

    /* Create our own GLib Main Context and make it the default one */
    context = g_main_context_new ();
    g_main_context_push_thread_default(context);

    /* Create the pipeline and elements */
    pipeline = gst_pipeline_new("pipeline");
    self->pipeline = pipeline;

    self->rtspsrc = gst_element_factory_make("rtspsrc", "source");
    self->rtph264depay = gst_element_factory_make("rtph264depay", "depay");
    self->queue = gst_element_factory_make("queue", "queue");
    self->h264parse = gst_element_factory_make("h264parse", "parse");
    self->avdec_h264 = gst_element_factory_make("avdec_h264", "decoder");
    self->autovideosink = gst_element_factory_make("autovideosink", "videosink");

    if (!pipeline || !self->rtspsrc || !self->rtph264depay || !self->queue || !self->h264parse || !self->avdec_h264 || !self->autovideosink) {
        gchar *message = g_strdup_printf("Not all elements could be created.");
        [self setUIMessage:message];
        g_free(message);
        return;
    }

    /* Set element properties */
    g_object_set(self->rtspsrc, "location", "rtsp://10.0.0.12:8554/camera.rlc_520a_clear", NULL);
    g_object_set(self->rtspsrc, "protocols", GST_RTSP_LOWER_TRANS_TCP, NULL);

    /* Add elements to the pipeline */
    gst_bin_add_many(GST_BIN(pipeline), self->rtspsrc, self->rtph264depay, self->queue, self->h264parse, self->avdec_h264, self->autovideosink, NULL);

    /* Link the elements (except rtspsrc, which is linked dynamically) */
    if (!gst_element_link_many(self->rtph264depay, self->queue, self->h264parse, self->avdec_h264, self->autovideosink, NULL)) {
        gchar *message = g_strdup_printf("Elements could not be linked.");
        [self setUIMessage:message];
        g_free(message);
        gst_object_unref(pipeline);
        return;
    }

    /* Connect to the pad-added signal for dynamic pad linking */
    g_signal_connect(self->rtspsrc, "pad-added", G_CALLBACK(on_pad_added), (__bridge void *)self);

    /* Set the pipeline to READY, so it can already accept a window handle */
    gst_element_set_state(pipeline, GST_STATE_READY);

    /* Set the video sink */
    self->video_sink = gst_bin_get_by_interface(GST_BIN(pipeline), GST_TYPE_VIDEO_OVERLAY);
    if (!self->video_sink) {
        GST_ERROR ("Could not retrieve video sink");
        return;
    }
    gst_video_overlay_set_window_handle(GST_VIDEO_OVERLAY(self->video_sink), (guintptr) (id) ui_video_view);

    /* Signals to watch */
    bus = gst_element_get_bus (pipeline);
    bus_source = gst_bus_create_watch (bus);
    g_source_set_callback (bus_source, (GSourceFunc) gst_bus_async_signal_func, NULL, NULL);
    g_source_attach (bus_source, context);
    g_source_unref (bus_source);
    g_signal_connect (G_OBJECT (bus), "message::error", (GCallback)error_cb, (__bridge void *)self);
    g_signal_connect (G_OBJECT (bus), "message::eos", (GCallback)eos_cb, (__bridge void *)self);
    g_signal_connect (G_OBJECT (bus), "message::state-changed", (GCallback)state_changed_cb, (__bridge void *)self);
    gst_object_unref (bus);

    /* Create a GLib Main Loop and set it to run */
    printf("\nEntering main loop...\n");
    main_loop = g_main_loop_new (context, FALSE);
    [self check_initialization_complete];
    g_main_loop_run (main_loop);
    GST_DEBUG ("Exited main loop");
    g_main_loop_unref (main_loop);
    main_loop = NULL;

    /* Free resources */
    g_main_context_pop_thread_default(context);
    g_main_context_unref (context);
    gst_element_set_state (pipeline, GST_STATE_NULL);
    gst_object_unref (pipeline);
    return;
}

@end
