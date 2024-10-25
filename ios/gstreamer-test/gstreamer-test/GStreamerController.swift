import Foundation
import UIKit

typealias GMainLoop = OpaquePointer
typealias gboolean = Int32

class GStreamerController: NSObject, ObservableObject {
    // Define constants for TRUE and FALSE
    let TRUE: gboolean = 1
    let FALSE: gboolean = 0

    // Define GType constants
    let G_TYPE_INT: GType = GType(6 << 2)
    let G_TYPE_UINT: GType = GType(7 << 2)
    let G_TYPE_BOOLEAN: GType = GType(5 << 2)
    let G_TYPE_STRING: GType = GType(16 << 2)
    let G_TYPE_BOXED: GType = GType(18 << 2)

    var pipeline: UnsafeMutablePointer<GstElement>?
    var bus: UnsafeMutablePointer<GstBus>?
    var mainLoop: GMainLoop?
    weak var videoView: UIView?

    override init() {
        super.init()
    }
    
    func startPipeline() {
        DispatchQueue.global(qos: .background).async {
            setenv("GST_DEBUG", "2", 1)
            gst_ios_init()
            self.setupPipeline()
            self.runMainLoop()
        }
    }
    
    func stopPipeline() {
        if let pipeline = self.pipeline {
            gst_element_set_state(pipeline, GST_STATE_NULL)
            gst_object_unref(pipeline)
            self.pipeline = nil
        }
        if let mainLoop = self.mainLoop {
            g_main_loop_quit(mainLoop)
            self.mainLoop = nil
        }
    }
    
    private func setupPipeline() {
        // Create elements
        guard let pipeline = gst_pipeline_new("pipeline"),
              let source = gst_element_factory_make("rtspsrc", "source"),
              let depay = gst_element_factory_make("rtph264depay", "depay"),
              let queue = gst_element_factory_make("queue", "queue"),
              let parse = gst_element_factory_make("h264parse", "parse"),
              let decoder = gst_element_factory_make("vtdec", "decoder"),
              let videorate = gst_element_factory_make("videorate", "videorate"),
              let videoscale = gst_element_factory_make("videoscale", "videoscale"),
              let capsfilter = gst_element_factory_make("capsfilter", "capsfilter"),
              let identity = gst_element_factory_make("identity", "identity"),
              let sink = gst_element_factory_make("glimagesink", "videosink")
        else {
            print("Failed to create elements")
            return
        }

        self.pipeline = pipeline

        // Set element properties using helper function
        setGObjectProperty(object: source, propertyName: "location", value: "rtsp://10.0.0.12:8554/camera.rlc_520a_clear")
        setGObjectProperty(object: source, propertyName: "protocols", value: Int32(4))
        setGObjectProperty(object: source, propertyName: "latency", value: Int32(1000))

        setGObjectProperty(object: identity, propertyName: "silent", value: FALSE)
        setGObjectProperty(object: sink, propertyName: "force-aspect-ratio", value: TRUE)
        setGObjectProperty(object: sink, propertyName: "render-rectangle", value: "<0,0,2560,1920>")

        // Set caps for capsfilter
        if let caps = gst_caps_from_string("video/x-raw,width=2560,height=1920") {
            setGObjectProperty(object: capsfilter, propertyName: "caps", value: caps)
            gst_caps_unref(caps)
        }

        let bin = UnsafeMutablePointer<GstBin>(OpaquePointer(pipeline))
        
        // Add elements to the pipeline individually
        if let pipeline = self.pipeline {
            let bin = UnsafeMutablePointer<GstBin>(OpaquePointer(pipeline))
            gst_bin_add(bin, source)
            gst_bin_add(bin, depay)
            gst_bin_add(bin, queue)
            gst_bin_add(bin, parse)
            gst_bin_add(bin, decoder)
            gst_bin_add(bin, videorate)
            gst_bin_add(bin, videoscale)
            gst_bin_add(bin, capsfilter)
            gst_bin_add(bin, identity)
            gst_bin_add(bin, sink)
        } else {
            print("Pipeline is not set")
            return
        }

        // Link elements individually (excluding rtspsrc which has dynamic pads)
        if gst_element_link(depay, queue) != TRUE ||
           gst_element_link(queue, parse) != TRUE ||
           gst_element_link(parse, decoder) != TRUE ||
           gst_element_link(decoder, videorate) != TRUE ||
           gst_element_link(videorate, videoscale) != TRUE ||
           gst_element_link(videoscale, capsfilter) != TRUE ||
           gst_element_link(capsfilter, identity) != TRUE ||
           gst_element_link(identity, sink) != TRUE {
            print("Failed to link elements")
            return
        }

        // Connect to the pad-added signal of rtspsrc
        g_signal_connect_data(source, "pad-added", unsafeBitCast(padAddedHandler, to: GCallback.self), UnsafeMutableRawPointer(mutating: depay), nil, GConnectFlags(0))

        // Set up bus to listen for messages
        self.bus = gst_element_get_bus(pipeline)
        gst_bus_add_watch(self.bus, { bus, message, data -> gboolean in
            let controller = Unmanaged<GStreamerController>.fromOpaque(data!).takeUnretainedValue()
            return controller.busCall(bus: bus, message: message, user_data: data)
        }, Unmanaged.passUnretained(self).toOpaque())

        // Set the pipeline to playing state
        let ret = gst_element_set_state(pipeline, GST_STATE_PLAYING)
        if ret == GST_STATE_CHANGE_FAILURE {
            print("Failed to set pipeline to PLAYING state")
            return
        }

        // Set the window handle after setting the pipeline to PLAYING
        DispatchQueue.main.async {
            self.setWindowHandle()
        }
    }

    // Pad-added handler function
    private let padAddedHandler: @convention(c) (UnsafeMutablePointer<GstElement>?, UnsafeMutablePointer<GstPad>?, gpointer?) -> Void = { (src, newPad, userData) in
        guard let depayPointer = userData else { return }
        let depay = depayPointer.assumingMemoryBound(to: GstElement.self)
        guard let newPad = newPad else { return }

        guard let sinkPad = gst_element_get_static_pad(depay, "sink") else {
            print("Failed to get sink pad of depay")
            return
        }

        defer { gst_object_unref(sinkPad) }

        if gst_pad_is_linked(sinkPad) > 0 {
            print("Sink pad already linked")
            return
        }

        // Attempt to link the new pad to the depayloader's sink pad
        let ret = gst_pad_link(newPad, sinkPad)
        if ret != GST_PAD_LINK_OK {
            print("Failed to link new pad to depay sink pad")
        } else {
            print("Linked new pad to depay sink pad")
        }
    }
    

    private func setGObjectProperty(object: UnsafeMutablePointer<GstElement>, propertyName: String, value: Any) {
        // Cast object to GObject
        let gobject = UnsafeMutableRawPointer(object).assumingMemoryBound(to: GObject.self)
        
        var gvalue = GValue()
        memset(&gvalue, 0, MemoryLayout<GValue>.size)
        
        // Access the GTypeInstance
        let typeInstance = UnsafeMutableRawPointer(gobject).assumingMemoryBound(to: GTypeInstance.self)
        
        // Access the class structure (GTypeClass*)
        guard let typeClass = typeInstance.pointee.g_class else {
            print("Failed to get object class for property \(propertyName)")
            return
        }
        
        // Cast GTypeClass* to GObjectClass*
        let objectClass = UnsafeMutableRawPointer(typeClass).assumingMemoryBound(to: GObjectClass.self)
        
        // Now use objectClass with g_object_class_find_property
        guard let property = g_object_class_find_property(objectClass, propertyName) else {
            print("Property \(propertyName) not found")
            return
        }
        
        let propertyType = property.pointee.value_type
        g_value_init(&gvalue, propertyType)
        
        switch propertyType {
        case G_TYPE_INT:
            if let intValue = value as? Int32 {
                g_value_set_int(&gvalue, intValue)
            } else {
                print("Invalid value type for property \(propertyName)")
            }
        case G_TYPE_UINT:
            if let uintValue = value as? UInt32 {
                g_value_set_uint(&gvalue, uintValue)
            } else {
                print("Invalid value type for property \(propertyName)")
            }
        case G_TYPE_BOOLEAN:
            if let boolValue = value as? gboolean {
                g_value_set_boolean(&gvalue, boolValue)
            } else {
                print("Invalid value type for property \(propertyName)")
            }
        case G_TYPE_STRING:
            if let stringValue = value as? String {
                g_value_set_string(&gvalue, stringValue)
            } else {
                print("Invalid value type for property \(propertyName)")
            }
        case G_TYPE_BOXED:
            // For properties like 'caps' which expect a boxed type
            if let boxedValue = value as? UnsafeMutableRawPointer {
                g_value_set_boxed(&gvalue, boxedValue)
            } else {
                print("Invalid value type for property \(propertyName)")
            }
        default:
            print("Unsupported property type for \(propertyName)")
            return
        }
        
        g_object_set_property(gobject, propertyName, &gvalue)
        g_value_unset(&gvalue)
    }
    
    private func runMainLoop() {
        self.mainLoop = g_main_loop_new(nil, gboolean(0))
        g_main_loop_run(self.mainLoop)
    }
    
    private func busCall(bus: UnsafeMutablePointer<GstBus>?,
                         message: UnsafeMutablePointer<GstMessage>?,
                         user_data: gpointer?) -> gboolean
    {
        guard let message = message else { return 0 }
        
        let messageType = message.pointee.type
        let messageTypeName = String(cString: gst_message_type_get_name(messageType))
//        print("GStreamer Message Type: \(messageTypeName)")
        
        switch messageType {
        case GST_MESSAGE_ERROR:
            var err: UnsafeMutablePointer<GError>?
            var debug: UnsafeMutablePointer<gchar>?
            gst_message_parse_error(message, &err, &debug)
            if let err = err {
                let errorMessage = String(cString: err.pointee.message)
                print("GStreamer Error: \(errorMessage)")
                g_error_free(err)
            }
            if let debug = debug {
                let debugInfo = String(cString: debug)
                print("GStreamer Debug Info: \(debugInfo)")
                g_free(debug)
            }
            gst_element_set_state(self.pipeline, GST_STATE_NULL)
            g_main_loop_quit(self.mainLoop)
        case GST_MESSAGE_EOS:
            print("GStreamer End of Stream")
            gst_element_set_state(self.pipeline, GST_STATE_NULL)
            g_main_loop_quit(self.mainLoop)
        default:
            break
        }
        return 1
    }
    
    private func setWindowHandle() {
        guard let videoView = self.videoView else {
            print("No video view to set window handle")
            return
        }
        guard let pipeline = self.pipeline else {
            print("Pipeline is not set")
            return
        }

        // Cast pipeline to GstBin*
        let bin = UnsafeMutablePointer<GstBin>(OpaquePointer(pipeline))

        // Retrieve the video sink element by name
        if let videoSinkElement = gst_bin_get_by_name(bin, "videosink") {

            // Check if the element implements GstVideoOverlay
            if let instance = UnsafeMutableRawPointer(videoSinkElement)?.assumingMemoryBound(to: GTypeInstance.self),
               g_type_check_instance_is_a(instance, gst_video_overlay_get_type()) != 0
            {

                let windowHandle = guintptr(bitPattern: Unmanaged.passUnretained(videoView).toOpaque())
                gst_video_overlay_set_window_handle(OpaquePointer(videoSinkElement), windowHandle)
                print("Window handle set for video sink")
            } else {
                print("Video sink does not support video overlay")
            }

            // Unreference the video sink element
            gst_object_unref(videoSinkElement)
        } else {
            print("Could not retrieve video sink to set window handle")
        }
    }
}

