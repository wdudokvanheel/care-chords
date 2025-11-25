#import "GStreamerBackend.h"
#import "gst_ios_init.h"
#import "SleepStream-Bridging-Header.h"
#import "Care_Chords-Swift.h"

GST_DEBUG_CATEGORY_STATIC (debug_category);
#define GST_CAT_DEFAULT debug_category

@implementation GStreamerBackend

-(id) init:(id) uiDelegate
{
    if (self = [super init])
    {
        self.ui_delegate = (id<GStreamerBackendDelegate>)uiDelegate;

        GST_DEBUG_CATEGORY_INIT (debug_category, "SleepStreamer", 0, "SleepStreamer-Backend");
        gst_debug_set_threshold_for_name("SleepStreamer", GST_LEVEL_TRACE);
    }

    return self;
}

-(void) run_app_pipeline_threaded
{
    dispatch_async(dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT, 0), ^{
           [self run_app_pipeline];
       });
}

-(void) play
{
    if(gst_element_set_state(self.pipeline, GST_STATE_PLAYING) == GST_STATE_CHANGE_FAILURE) {
        [self setUIMessage:"Failed to set pipeline to playing"];
    }
}

-(void) pause
{
    if(gst_element_set_state(self.pipeline, GST_STATE_PAUSED) == GST_STATE_CHANGE_FAILURE) {
        [self setUIMessage:"Failed to set pipeline to paused"];
    }
}

-(void) stop
{
    if (self.main_loop) {
        g_main_loop_quit(self.main_loop);
    }
}

-(void) setUIMessage:(gchar*) message
{
    printf("Setting message to: %s\n", message);
    NSString *messagString = [NSString stringWithUTF8String:message];
    if(self.ui_delegate)
    {
        dispatch_async(dispatch_get_main_queue(), ^{
            [self.ui_delegate gstreamerMessageWithMessage:messagString];
        });
    }
}

-(void) check_initialization_complete
{
    if (!self.initialized && self.main_loop) {
        GST_DEBUG ("Initialization complete, notifying application.");
        if (self.ui_delegate)
        {
            dispatch_async(dispatch_get_main_queue(), ^{
                [self.ui_delegate gStreamerInitialized];
            });
        }
        self.initialized = TRUE;
    }
}

// Default implementation, should be overridden or extended
-(void) run_app_pipeline
{
    GSource *bus_source;
    GstBus *bus;

    GST_DEBUG ("Creating pipeline");

    /* Create our own GLib Main Context and make it the default one */
    self.context = g_main_context_new ();
    g_main_context_push_thread_default(self.context);

    /* Build the pipeline */
    [self build_pipeline];
    
    if (!self.pipeline) {
        // build_pipeline should have set the error message
        return;
    }

    /* Signals to watch */
    bus = gst_element_get_bus (self.pipeline);
    bus_source = gst_bus_create_watch (bus);
    g_source_set_callback (bus_source, (GSourceFunc) gst_bus_async_signal_func, NULL, NULL);
    g_source_attach (bus_source, self.context);
    g_source_unref (bus_source);
    
    // We need to pass 'self' to the callbacks. Since these are C functions, we need to be careful.
    // We can define static functions in this file that cast user_data back to GStreamerBackend*
    g_signal_connect (G_OBJECT (bus), "message::error", (GCallback)error_cb, (__bridge void *)self);
    g_signal_connect (G_OBJECT (bus), "message::eos", (GCallback)eos_cb, (__bridge void *)self);
    g_signal_connect (G_OBJECT (bus), "message::state-changed", (GCallback)state_changed_cb, (__bridge void *)self);
    gst_object_unref (bus);

    /* Create and run the main loop */
    GST_DEBUG ("Starting main loop...");
    self.main_loop = g_main_loop_new (self.context, FALSE);
    [self check_initialization_complete];
    g_main_loop_run (self.main_loop);
    GST_DEBUG ("Main loop finished");
    g_main_loop_unref (self.main_loop);
    self.main_loop = NULL;

    /* Free resources */
    g_main_context_pop_thread_default(self.context);
    g_main_context_unref (self.context);
    gst_element_set_state (self.pipeline, GST_STATE_NULL);
    gst_object_unref (self.pipeline);
    self.pipeline = NULL;
}

-(void) build_pipeline {
    // To be overridden
}

/* Static callbacks */

static void eos_cb(GstBus *bus, GstMessage *msg, GStreamerBackend *self){
    printf("\nEOS called\n");
    gst_element_set_state (self.pipeline, GST_STATE_NULL);
    g_main_loop_quit(self.main_loop);
}

static void error_cb (GstBus *bus, GstMessage *msg, GStreamerBackend *self)
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
    gst_element_set_state (self.pipeline, GST_STATE_NULL);
}

static void state_changed_cb (GstBus *bus, GstMessage *msg, GStreamerBackend *self)
{
    GstState old_state, new_state, pending_state;
    gst_message_parse_state_changed (msg, &old_state, &new_state, &pending_state);

    if (GST_MESSAGE_SRC (msg) == GST_OBJECT (self.pipeline)) {
        printf("State changed from %s to %s\n", gst_element_state_get_name(old_state), gst_element_state_get_name(new_state));

        // Subclasses might want to handle state changes specifically, but for now we can do generic logging or delegate calls if needed.
        // The original audio backend had specific delegate calls here. We might need to expose a method to handle this or keep it generic.
        // For now, let's replicate the generic behavior or check if the delegate responds to specific selectors if we were to make them optional,
        // but the current delegates have specific methods.
        
        // We can check if the delegate conforms to Audio or Video protocols to call specific methods,
        // OR we can make the delegate methods more generic.
        // Given the plan was to unify, let's see if we can make the delegate methods generic or handle them in subclasses.
        // However, since this is a base class, we can't easily call subclass-specific delegate methods without casting.
        // A better approach might be to have a virtual method `stateChanged:old:new:pending:` that subclasses override.
        
        [self stateChanged:new_state old:old_state pending:pending_state];
    }
}

-(void) stateChanged:(GstState)newState old:(GstState)oldState pending:(GstState)pendingState {
    // Default implementation does nothing or generic logging
}

@end
