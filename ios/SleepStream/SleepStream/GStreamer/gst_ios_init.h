#ifndef __GST_IOS_INIT_H__
#define __GST_IOS_INIT_H__

#include <gst/gst.h>

G_BEGIN_DECLS

#define GST_G_IO_MODULE_DECLARE(name) \
extern void G_PASTE(g_io_, G_PASTE(name, _load)) (gpointer module)

#define GST_G_IO_MODULE_LOAD(name) \
G_PASTE(g_io_, G_PASTE(name, _load)) (NULL)

/* Uncomment each line to enable the plugin categories that your application needs.
 * You can also enable individual plugins. See gst_ios_init.c to see their names
 */

#define GST_IOS_PLUGIN_COREELEMENTS
#define GST_IOS_PLUGIN_APP
#define GST_IOS_PLUGIN_AUDIOCONVERT
#define GST_IOS_PLUGIN_AUDIORESAMPLE
#define GST_IOS_PLUGIN_AUTODETECT
#define GST_IOS_PLUGIN_VIDEOCONVERTSCALE
#define GST_IOS_PLUGIN_AUDIOPARSERS
#define GST_IOS_PLUGIN_VIDEOPARSERSBAD
#define GST_IOS_PLUGIN_TCP
#define GST_IOS_PLUGIN_UDP
#define GST_IOS_PLUGIN_RTSP
#define GST_IOS_PLUGIN_RTP
#define GST_IOS_PLUGIN_RTPMANAGER
#define GST_IOS_PLUGIN_SDPELEM
#define GST_IOS_PLUGIN_OSXAUDIO
#define GST_IOS_PLUGIN_VIDEOCROP
#define GST_IOS_PLUGIN_LIBAV


//#define GST_IOS_GIO_MODULE_GNUTLS

void gst_ios_init (void);

G_END_DECLS

#endif
