import Foundation
import UIKit

typealias GMainLoop = OpaquePointer
typealias gboolean = Int32

class GStreamerController: NSObject, ObservableObject {
    var pipeline: UnsafeMutablePointer<GstElement>?
    var bus: UnsafeMutablePointer<GstBus>?
    var mainLoop: GMainLoop?
    weak var videoView: UIView?

    override init() {
        super.init()
    }
    
    func startPipeline() {
        DispatchQueue.global(qos: .background).async {
            setenv("GST_DEBUG", "3", 1)
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
        // Define your pipeline
        let pipelineDescription = "rtspsrc location=rtsp://10.0.0.21:8554/lumi protocols=tcp ! rtph264depay ! queue ! h264parse ! avdec_h264 ! videoconvert ! videoscale !  video/x-raw,width=640,height=480 ! glimagesink name=videosink"
        
        // Parse the pipeline
        var error: UnsafeMutablePointer<GError>?
        self.pipeline = gst_parse_launch(pipelineDescription, &error)
        
        if let error = error {
            let errorMessage = String(cString: error.pointee.message)
            print("GStreamer Error: \(errorMessage)")
            g_error_free(error)
            return
        }
        
        // Set up bus to listen for messages
        if let pipeline = self.pipeline {
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
        print("GStreamer Message Type: \(messageTypeName)")
        
        switch messageType {
//        case GST_MESSAGE_ELEMENT:
//            if let structure = gst_message_get_structure(message) {
//                let name = String(cString: gst_structure_get_name(structure))
//                if name == "prepare-window-handle" {
//                    DispatchQueue.main.async {
//                        self.setWindowHandle(message: message)
//                    }
//                }
//            }
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
               g_type_check_instance_is_a(instance, gst_video_overlay_get_type()) != 0 {

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
