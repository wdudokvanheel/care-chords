#import <unistd.h>
#import "GStreamerAudioBackend.h"
#import "gst_ios_init.h"
#import <UIKit/UIKit.h>

#import <GStreamer/gst/gst.h>
#import <GStreamer/gst/rtsp/rtsp.h>
#import "SleepStream-Bridging-Header.h"

GST_DEBUG_CATEGORY_STATIC (debug_category);
#define GST_CAT_DEFAULT debug_category

#import "SleepStream-Swift.h"

@interface GStreamerAudioBackend()
-(void)setUIMessage:(gchar*) message;
-(void)run_app_pipeline;
-(void)check_initialization_complete;
@end

@implementation GStreamerAudioBackend {
    id<GStreamerBackendDelegate> ui_delegate;        /* Class that we use to interact with the user interface */
    GstElement *pipeline;      /* The running pipeline */
    GMainContext *context;     /* GLib context used to run the main loop */
    GMainLoop *main_loop;      /* GLib main loop */
    gboolean initialized;      /* To avoid informing the UI multiple times about the initialization */
    GstBus *bus;
    GstMessage* eos_msg;

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
    if (self = [super init])
    {
        self->ui_delegate = (id<GStreamerBackendDelegate>)uiDelegate;

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
    printf("PAUSING!!!!!!\n\n\n\n\n\n\n");
    if(gst_element_set_state(pipeline, GST_STATE_PAUSED) == GST_STATE_CHANGE_FAILURE) {
        [self setUIMessage:"Failed to set pipeline to paused"];
    }
    else{
        gst_element_seek(pipeline, 1.0, GST_FORMAT_TIME, GST_SEEK_FLAG_FLUSH, GST_SEEK_TYPE_SET, 0, GST_SEEK_TYPE_NONE, GST_CLOCK_TIME_NONE);
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

static void eos_cb(GstBus *bus, GstMessage *msg, GStreamerAudioBackend *self){
    printf("\nEOS called\n");
    gst_element_set_state (self->pipeline, GST_STATE_NULL);
    g_main_loop_quit(self->main_loop);
}

/* Retrieve errors from the bus and show them on the UI */
static void error_cb (GstBus *bus, GstMessage *msg, GStreamerAudioBackend *self)
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
static void state_changed_cb (GstBus *bus, GstMessage *msg, GStreamerAudioBackend *self)
{
    GstState old_state, new_state, pending_state;
    gst_message_parse_state_changed (msg, &old_state, &new_state, &pending_state);

    /* Only pay attention to messages coming from the pipeline, not its children */
    if (GST_MESSAGE_SRC (msg) == GST_OBJECT (self->pipeline)) {
        printf("State changed from %s to %s\n", gst_element_state_get_name(old_state), gst_element_state_get_name(new_state));

        switch (new_state) {
            case GST_STATE_PLAYING:
                if (self->ui_delegate) {
                    [self->ui_delegate gstreamerAudioStateWithState:AudioStatePlaying];
                }
                break;

            case GST_STATE_PAUSED:
                if (self->ui_delegate) {
                    [self->ui_delegate gstreamerAudioStateWithState:(AudioState)AudioStatePaused];
                }
                break;

            case GST_STATE_READY:
                if (self->ui_delegate) {
                    [self->ui_delegate gstreamerAudioStateWithState:(AudioState)AudioStateReady];
                }
                break;
            case GST_STATE_NULL:
                if (self->ui_delegate) {
                    [self->ui_delegate gstreamerAudioStateWithState:(AudioState)AudioStateInitializing];
                }
                break;

            default:
                break;
        }
    }
    // TODO Check state of audio sink to know actual playing state?
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
    self->depayloader = gst_element_factory_make("rtpmp4adepay", "depay");
    self->queue = gst_element_factory_make("queue", "queue");
    self->parser = gst_element_factory_make("aacparse", "parser");
    self->decoder = gst_element_factory_make("avdec_aac", "decoder");
    self->converter = gst_element_factory_make("audioconvert", "converter");
    self->sampler = gst_element_factory_make("audioresample", "sampler");
    self->audio_sink = gst_element_factory_make("autoaudiosink", "audiosink");

    if (!pipeline || !self->rtspsrc || !self->depayloader || !self->queue || !self->parser || !self->decoder || !self->audio_sink || !self->converter || !self->sampler) {
        gchar *message = g_strdup_printf("Not all elements could be created.");
        [self setUIMessage:message];
        g_free(message);
        return;
    }

    /* Set element properties */
    g_object_set(self->rtspsrc, "location", "rtsp://10.0.0.153:8554/sleep", NULL);
    g_object_set(self->rtspsrc, "protocols", GST_RTSP_LOWER_TRANS_TCP, NULL);

    /* Add elements to the pipeline */
    gst_bin_add_many(GST_BIN(pipeline), self->rtspsrc, self->depayloader, self->queue, self->parser, self->decoder, self->converter, self->sampler, self->audio_sink, NULL);

    /* Link the elements (except rtspsrc, which is linked dynamically) */
    if (!gst_element_link_many(self->depayloader, self->queue, self->parser, self->decoder, self->converter, self->sampler, self->audio_sink, NULL)) {
        gchar *message = g_strdup_printf("Elements could not be linked.");
        [self setUIMessage:message];
        g_free(message);
        gst_object_unref(pipeline);
        return;
    }

    /* Connect to the pad-added signal for dynamic pad linking */
    g_signal_connect(self->rtspsrc, "pad-added", G_CALLBACK(on_pad_added), (__bridge void *)self);

    /* Set the pipeline to READY */
    gst_element_set_state(pipeline, GST_STATE_READY);

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

    /* Create and run the main loop */
    GST_DEBUG ("\Starting main loop...\n");
    main_loop = g_main_loop_new (context, FALSE);
    [self check_initialization_complete];
    g_main_loop_run (main_loop);
    GST_DEBUG ("Main loop finished");
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
