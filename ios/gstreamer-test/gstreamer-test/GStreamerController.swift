import AVFoundation
import Foundation
import UIKit

typealias GMainLoop = OpaquePointer
typealias gboolean = Int32

class GStreamerController: NSObject, ObservableObject {
    let glQueue = DispatchQueue(label: "com.bitechular.glContextQueue")
    let streamQueue = DispatchQueue(label: "com.bitechular.streamtQueue")

    var video: Bool

    // Define constants for TRUE and FALSE
    let TRUE: gboolean = 1
    let FALSE: gboolean = 0

    // Define GType constants
    let G_TYPE_INT: GType = .init(6 << 2)
    let G_TYPE_UINT: GType = .init(7 << 2)
    let G_TYPE_BOOLEAN: GType = .init(5 << 2)
    let G_TYPE_STRING: GType = .init(16 << 2)
    let G_TYPE_BOXED: GType = .init(18 << 2)

    var pipeline: UnsafeMutablePointer<GstElement>?
    var bus: UnsafeMutablePointer<GstBus>?
    var mainLoop: GMainLoop?
    weak var videoView: UIView?
    var bin: UnsafeMutablePointer<GstBin>?
    var padAddedSignalId: gulong = 0

    var videosink: UnsafeMutablePointer<GstElement>?
    var first = true

    init(video: Bool) {
        self.video = video
        super.init()
        configureAudioSession()
        setenv("GST_DEBUG", "4", 1)
        streamQueue.async {
            gst_ios_init()
        }
        startPipeline()
    }

    func startPipeline() {
        streamQueue.asyncAfter(deadline: .now() + 3.0) { [self] in
            setupPipeline()
            runMainLoop()
        }
    }

    func configureAudioSession() {
        let session = AVAudioSession.sharedInstance()
        do {
            try session.setCategory(.playback, mode: .default, options: [])
            try session.setActive(true)
        } catch {
            print("Failed to set up audio session: \(error)")
        }
    }

    func stopPipeline() {
//        self.videoView = nil
        glQueue.async { [self] in
            print("Stopping")
            DispatchQueue.main.sync { [self] in
                unsetWindowHandle()
                if let videoSink = gst_bin_get_by_name(bin, "videosink") {
                    gst_element_set_state(videoSink, GST_STATE_NULL)
                    gst_bin_remove(bin, videoSink)
                    gst_object_unref(videoSink)
                }

                if let pipeline = pipeline {
                    gst_element_set_state(pipeline, GST_STATE_NULL)
                    gst_object_unref(pipeline)
                    self.pipeline = nil
                } else {
                    print("no pipeline")
                }

                if let bus = bus {
                    gst_object_unref(bus)
                    self.bus = nil
                }
                if let mainLoop = mainLoop {
                    g_main_loop_unref(mainLoop)
                    self.mainLoop = nil
                }
            }
        }
    }

//    func stopPipelineAndWait() {
//        print("STOPPING AND WAITING XXX")
//        glQueue.async { [self] in
//            video.toggle()
//            print("XXX STOPPING!")
//            unsetWindowHandle()
//            stopPipeline()
//        }
//
//        print("OK")
//        // Adding a delay before restarting the pipeline
//        startPipeline()
//    }

    private func unsetWindowHandle() {
        print("UNSET XXXXX")
        guard let pipeline = pipeline else {
            print("Pipeline is not set XXXX")
            return
        }

        let bin = UnsafeMutablePointer<GstBin>(OpaquePointer(pipeline))
        if let videoSinkElement = gst_bin_get_by_name(bin, "videosink") {
            if let instance = UnsafeMutableRawPointer(videoSinkElement)?.assumingMemoryBound(to: GTypeInstance.self),
               g_type_check_instance_is_a(instance, gst_video_overlay_get_type()) != 0
            {
                gst_video_overlay_set_window_handle(OpaquePointer(videoSinkElement), 0)
                print("Window handle unset for video sink XXXX")
            } else {
                print("XXXX FAILED")
            }
            gst_object_unref(videoSinkElement)
        } else {
            print("XXX NOT FOUND")
        }
    }

    private func setupPipeline() {
        gst_init(nil, nil)

        // Create elements
        guard let pipeline = gst_pipeline_new("pipeline"),
              let source = gst_element_factory_make("rtspsrc", "source")
        else {
            print("Failed to create elements")
            return
        }

        self.pipeline = pipeline

        // Set element properties
        setGObjectProperty(object: source, propertyName: "location", value: "rtsp://10.0.0.21:8554/lumi")
        setGObjectProperty(object: source, propertyName: "protocols", value: Int32(4)) // GstRTSPLowerTrans.TCP
        setGObjectProperty(object: source, propertyName: "latency", value: Int32(200))

        bin = UnsafeMutablePointer<GstBin>(OpaquePointer(pipeline))
        gst_bin_add(bin, source)

        if video {
            // Video elements
            guard let videoDepay = gst_element_factory_make("rtph264depay", "videoDepay"),
                  let videoParse = gst_element_factory_make("h264parse", "videoParse"),
                  let videoDecoder = gst_element_factory_make("vtdec", "videoDecoder"),
                  let videoSink = gst_element_factory_make("glimagesink", "videosink")
            else {
                print("Failed to create video elements")
                return
            }

            // Set video sink properties
            setGObjectProperty(object: videoSink, propertyName: "force-aspect-ratio", value: TRUE)

            // Add video elements to pipeline
            gst_bin_add(bin, videoDepay)
            gst_bin_add(bin, videoParse)
            gst_bin_add(bin, videoDecoder)
            gst_bin_add(bin, videoSink)

            videosink = videoSink

            // Link video elements
            if gst_element_link(videoDepay, videoParse) != TRUE ||
                gst_element_link(videoParse, videoDecoder) != TRUE ||
                gst_element_link(videoDecoder, videoSink) != TRUE
            {
                print("Failed to link video elements")
                return
            }
        }

        // Audio elements
        guard let audioDepay = gst_element_factory_make("rtpmp4gdepay", "audioDepay"),
              let audioParse = gst_element_factory_make("aacparse", "audioParse"),
              let audioDecoder = gst_element_factory_make("avdec_aac", "audioDecoder"),
              let audioConvert = gst_element_factory_make("audioconvert", "audioConvert"),
              let audioResample = gst_element_factory_make("audioresample", "audioResample"),
              let audioSink = gst_element_factory_make("autoaudiosink", "audioSink")
        else {
            print("Failed to create audio elements")
            return
        }

        // Add audio elements to pipeline
        gst_bin_add(bin, audioDepay)
        gst_bin_add(bin, audioParse)
        gst_bin_add(bin, audioDecoder)
        gst_bin_add(bin, audioConvert)
        gst_bin_add(bin, audioResample)
        gst_bin_add(bin, audioSink)

        // Link audio elements
        if gst_element_link(audioDepay, audioParse) != TRUE ||
            gst_element_link(audioParse, audioDecoder) != TRUE ||
            gst_element_link(audioDecoder, audioConvert) != TRUE ||
            gst_element_link(audioConvert, audioResample) != TRUE ||
            gst_element_link(audioResample, audioSink) != TRUE
        {
            print("Failed to link audio elements")
            return
        }

        // Connect to the pad-added signal of rtspsrc
        padAddedSignalId = g_signal_connect_data(source, "pad-added", unsafeBitCast(padAddedCallback, to: GCallback.self), Unmanaged.passUnretained(self).toOpaque(), nil, GConnectFlags(0))
        // Connect to the pad-added signal of rtspsrc

        // Set up bus to listen for messages
        bus = gst_element_get_bus(pipeline)
        gst_bus_add_watch(bus, { bus, message, data -> gboolean in
            let controller = Unmanaged<GStreamerController>.fromOpaque(data!).takeUnretainedValue()
            return controller.busCall(bus: bus, message: message, user_data: data)
        }, Unmanaged.passUnretained(self).toOpaque())

        // Set the pipeline to playing state
        let ret = gst_element_set_state(pipeline, GST_STATE_PLAYING)
        if ret == GST_STATE_CHANGE_FAILURE {
            print("Failed to set pipeline to PLAYING state")
            return
        }

        if video {
            first = false
            // Set the window handle after setting the pipeline to PLAYING
            DispatchQueue.main.async {
                self.setWindowHandle()
            }
        }
    }

    func toggleVideo() {
        if let pipeline = pipeline {
            print("Stopping pipeline before toggling video")
            stopPipeline()
            // Delay to ensure resources are cleaned up properly
            DispatchQueue.global(qos: .background).asyncAfter(deadline: .now() + 1.0) {
                self.video.toggle()
                self.startPipeline()
            }
        } else {
            video.toggle()
            startPipeline()
        }
    }

    private func addVideoElements(to bin: UnsafeMutablePointer<GstBin>) {
        guard let videoDepay = gst_element_factory_make("rtph264depay", "videoDepay"),
              let videoParse = gst_element_factory_make("h264parse", "videoParse"),
              let videoDecoder = gst_element_factory_make("vtdec", "videoDecoder"),
              let videoSink = gst_element_factory_make("glimagesink", "videosink")
        else {
            print("Failed to create video elements")
            return
        }

        setGObjectProperty(object: videoSink, propertyName: "force-aspect-ratio", value: TRUE)

        gst_bin_add(bin, videoDepay)
        gst_bin_add(bin, videoParse)
        gst_bin_add(bin, videoDecoder)
        gst_bin_add(bin, videoSink)

        if gst_element_link(videoDepay, videoParse) != TRUE ||
            gst_element_link(videoParse, videoDecoder) != TRUE ||
            gst_element_link(videoDecoder, videoSink) != TRUE
        {
            print("Failed to link video elements")
            return
        }

        // Reconnect pad-added signal if necessary
        if padAddedSignalId == 0, let source = gst_bin_get_by_name(bin, "source") {
            padAddedSignalId = g_signal_connect_data(source, "pad-added", unsafeBitCast(padAddedCallback, to: GCallback.self), Unmanaged.passUnretained(self).toOpaque(), nil, GConnectFlags(0))
            gst_object_unref(source)
        }
    }

    private func removeVideoElements(from bin: UnsafeMutablePointer<GstBin>) {
        // Disconnect pad-added signal to avoid multiple pads being linked
        if padAddedSignalId != 0, let source = gst_bin_get_by_name(bin, "source") {
            g_signal_handler_disconnect(source, padAddedSignalId)
            padAddedSignalId = 0
            gst_object_unref(source)
        }

        if let videoSink = gst_bin_get_by_name(bin, "videosink") {
            gst_element_set_state(videoSink, GST_STATE_NULL)
            gst_bin_remove(bin, videoSink)
            gst_object_unref(videoSink)
        }
        if let videoDecoder = gst_bin_get_by_name(bin, "videoDecoder") {
            gst_element_set_state(videoDecoder, GST_STATE_NULL)
            gst_bin_remove(bin, videoDecoder)
            gst_object_unref(videoDecoder)
        }
        if let videoParse = gst_bin_get_by_name(bin, "videoParse") {
            gst_element_set_state(videoParse, GST_STATE_NULL)
            gst_bin_remove(bin, videoParse)
            gst_object_unref(videoParse)
        }
        if let videoDepay = gst_bin_get_by_name(bin, "videoDepay") {
            gst_element_set_state(videoDepay, GST_STATE_NULL)
            gst_bin_remove(bin, videoDepay)
            gst_object_unref(videoDepay)
        }
    }

    // Define the callback with the correct signature
    private let padAddedCallback: @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?) -> Void = { elementPtr, padPtr, userData in
        let element = elementPtr?.assumingMemoryBound(to: GstElement.self)
        let pad = padPtr?.assumingMemoryBound(to: GstPad.self)
        let controller = Unmanaged<GStreamerController>.fromOpaque(userData!).takeUnretainedValue()
        controller.handlePadAdded(src: element, newPad: pad)
    }

    private func handlePadAdded(src: UnsafeMutablePointer<GstElement>?, newPad: UnsafeMutablePointer<GstPad>?) {
        guard let newPad = newPad else { return }

        // Get the caps of the new pad
        let newPadCaps = gst_pad_get_current_caps(newPad)
        let newPadStruct = gst_caps_get_structure(newPadCaps, 0)
        let newPadType = String(cString: gst_structure_get_name(newPadStruct))

        if let srcElement = src {
            let gstObject = UnsafeMutablePointer<GstObject>(OpaquePointer(srcElement))
            if let name = gst_object_get_name(gstObject) {
                print("Received new pad '\(newPadType)' from '\(String(cString: name))'")
                g_free(UnsafeMutableRawPointer(mutating: name))
            } else {
                print("Received new pad '\(newPadType)' from unknown element")
            }
        }

        if newPadType.starts(with: "application/x-rtp") {
            if let mediaTypeCString = gst_structure_get_string(newPadStruct, "media") {
                let mediaTypeString = String(cString: mediaTypeCString)
                if mediaTypeString == "video" {
                    if !video {
                        return
                    }
                    // Link to video depayloader
                    let bin = UnsafeMutablePointer<GstBin>(OpaquePointer(pipeline))
                    guard let videoDepay = gst_bin_get_by_name(bin, "videoDepay") else {
                        print("Failed to get video depayloader")
                        return
                    }
                    let sinkPad = gst_element_get_static_pad(videoDepay, "sink")
                    if gst_pad_is_linked(sinkPad) > 0 {
                        print("Video sink pad already linked")
                        gst_object_unref(sinkPad)
                        gst_object_unref(videoDepay)
                        return
                    }
                    let ret = gst_pad_link(newPad, sinkPad)
                    if ret == GST_PAD_LINK_OK {
                        print("Linked video pad")
                    } else {
                        print("Failed to link video pad")
                    }
                    gst_object_unref(sinkPad)
                    gst_object_unref(videoDepay)
                } else if mediaTypeString == "audio" {
                    // Link to audio depayloader
                    let bin = UnsafeMutablePointer<GstBin>(OpaquePointer(pipeline))
                    guard let audioDepay = gst_bin_get_by_name(bin, "audioDepay") else {
                        print("Failed to get audio depayloader")
                        return
                    }
                    let sinkPad = gst_element_get_static_pad(audioDepay, "sink")
                    if gst_pad_is_linked(sinkPad) > 0 {
                        print("Audio sink pad already linked")
                        gst_object_unref(sinkPad)
                        gst_object_unref(audioDepay)
                        return
                    }
                    let ret = gst_pad_link(newPad, sinkPad)
                    if ret == GST_PAD_LINK_OK {
                        print("Linked audio pad")
                    } else {
                        print("Failed to link audio pad")
                    }
                    gst_object_unref(sinkPad)
                    gst_object_unref(audioDepay)
                }
            }
        }
        gst_caps_unref(newPadCaps)
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
        mainLoop = g_main_loop_new(nil, gboolean(0))
        print("start loop")
        g_main_loop_run(mainLoop)
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
            gst_element_set_state(pipeline, GST_STATE_NULL)
            g_main_loop_quit(mainLoop)
        case GST_MESSAGE_EOS:
            print("GStreamer End of Stream")
            gst_element_set_state(pipeline, GST_STATE_NULL)
            g_main_loop_quit(mainLoop)
        default:
            break
        }
        return 1
    }

    private func setWindowHandle() {
        print("WINDOWHANDLE XX")
        guard let videoView = videoView else {
            print("No video view to set window handle")
            return
        }
        guard let pipeline = pipeline else {
            print("Pipeline is not set")
            return
        }

        // Cast pipeline to GstBin*
        let bin = UnsafeMutablePointer<GstBin>(OpaquePointer(pipeline))
        print("WINDOWHANDLEX XX")

        // Retrieve the video sink element by name
        if let videoSinkElement = gst_bin_get_by_name(bin, "videosink") {
            print("WINDOWHANDLE XXXXX")

            // Check if the element implements GstVideoOverlay
            if let instance = UnsafeMutableRawPointer(videoSinkElement)?.assumingMemoryBound(to: GTypeInstance.self),
               g_type_check_instance_is_a(instance, gst_video_overlay_get_type()) != 0
            {
                print("WINDOWHANDLE XXXXX")

                let windowHandle = guintptr(bitPattern: Unmanaged.passUnretained(videoView).toOpaque())
                gst_video_overlay_set_window_handle(OpaquePointer(videoSinkElement), windowHandle)
                print("Window handle set for video sink")
            } else {
                print("Video sink does not support video overlay")
            }
            print("WINDOWHANDLE XXXXXXX")

            // Unreference the video sink element
            gst_object_unref(videoSinkElement)
        } else {
            print("Could not retrieve video sink to set window handle")
        }
    }
}
