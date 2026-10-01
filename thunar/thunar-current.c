#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <gtk/gtk.h>
#include <gio/gio.h>

typedef void (*orig_gtk_window_set_title_f)(GtkWindow *window, const gchar *title);
static orig_gtk_window_set_title_f orig_gtk_window_set_title = NULL;

__attribute__((constructor))
static void init(void) {
    unsetenv("LD_PRELOAD");
}

static char *json_escape(const char *s) {
    GString *out = g_string_new("\"");
    for (; *s; s++) {
        if (*s == '"') g_string_append(out, "\\\"");
        else if (*s == '\\') g_string_append(out, "\\\\");
        else if (*s == '\n') g_string_append(out, "\\n");
        else if (*s == '\r') g_string_append(out, "\\r");
        else if (*s == '\t') g_string_append(out, "\\t");
        else g_string_append_c(out, *s);
    }
    g_string_append_c(out, '"');
    return g_string_free(out, FALSE);
}

static void send_to_current(const char *path) {
    if (!path || !path[0]) return;

    const char *runtime_dir = getenv("XDG_RUNTIME_DIR");
    if (!runtime_dir) runtime_dir = "/run/user/1000";

    struct sockaddr_un addr;
    memset(&addr, 0, sizeof(addr));
    addr.sun_family = AF_UNIX;
    int path_len = snprintf(addr.sun_path, sizeof(addr.sun_path), "%s/current.sock", runtime_dir);
    if (path_len < 0 || (size_t)path_len >= sizeof(addr.sun_path)) return;

    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd < 0) return;

    if (connect(fd, (struct sockaddr *)&addr, sizeof(addr)) == 0) {
        char *esc_path = json_escape(path);
        char payload[4096];
        int len = snprintf(payload, sizeof(payload),
            "{\"type\":\"Publish\",\"attention\":{\"folder\":%s}}\n",
            esc_path
        );
        g_free(esc_path);
        if (len > 0) {
            ssize_t written = write(fd, payload, len);
            (void)written;
        }
    }
    close(fd);
}

static void publish_window(GtkWindow *window) {
    if (!window || !GTK_IS_WINDOW(window)) return;

    GType thunar_win_type = g_type_from_name("ThunarWindow");
    if (!thunar_win_type || !g_type_is_a(G_OBJECT_TYPE(window), thunar_win_type)) {
        return;
    }

    GObject *file = NULL;
    g_object_get(G_OBJECT(window), "current-directory", &file, NULL);
    if (!file) return;

    typedef gchar* (*thunarx_get_uri_f)(GObject*);
    static thunarx_get_uri_f thunarx_get_uri = NULL;
    if (!thunarx_get_uri) {
        thunarx_get_uri = (thunarx_get_uri_f)dlsym(RTLD_DEFAULT, "thunarx_file_info_get_uri");
    }

    if (thunarx_get_uri) {
        gchar *uri = thunarx_get_uri(file);
        if (uri) {
            gchar *path = g_filename_from_uri(uri, NULL, NULL);
            if (path) {
                send_to_current(path);
                g_free(path);
            }
            g_free(uri);
        }
    }
    g_object_unref(file);
}

static void on_notify_dir(GObject *gobject, GParamSpec *pspec, gpointer user_data) {
    (void)pspec;
    (void)user_data;
    publish_window(GTK_WINDOW(gobject));
}

static void on_notify_active(GObject *gobject, GParamSpec *pspec, gpointer user_data) {
    (void)pspec;
    (void)user_data;
    if (gtk_window_is_active(GTK_WINDOW(gobject))) {
        publish_window(GTK_WINDOW(gobject));
    }
}

static gboolean on_focus_in(GtkWidget *widget, GdkEventFocus *event, gpointer user_data) {
    (void)event;
    (void)user_data;
    publish_window(GTK_WINDOW(widget));
    return FALSE;
}

static gboolean on_map(GtkWidget *widget, GdkEvent *event, gpointer user_data) {
    (void)event;
    (void)user_data;
    publish_window(GTK_WINDOW(widget));
    return FALSE;
}

void gtk_window_set_title(GtkWindow *window, const gchar *title) {
    if (!orig_gtk_window_set_title) {
        orig_gtk_window_set_title = (orig_gtk_window_set_title_f)dlsym(RTLD_NEXT, "gtk_window_set_title");
    }
    if (orig_gtk_window_set_title) {
        orig_gtk_window_set_title(window, title);
    }

    if (window && GTK_IS_WINDOW(window)) {
        GType thunar_win_type = g_type_from_name("ThunarWindow");
        if (thunar_win_type && g_type_is_a(G_OBJECT_TYPE(window), thunar_win_type)) {
            if (!g_object_get_data(G_OBJECT(window), "current_hook_connected")) {
                g_object_set_data(G_OBJECT(window), "current_hook_connected", GINT_TO_POINTER(1));
                g_signal_connect(window, "notify::current-directory", G_CALLBACK(on_notify_dir), NULL);
                g_signal_connect(window, "notify::is-active", G_CALLBACK(on_notify_active), NULL);
                g_signal_connect(window, "focus-in-event", G_CALLBACK(on_focus_in), NULL);
                g_signal_connect(window, "map-event", G_CALLBACK(on_map), NULL);
            }
            publish_window(window);
        }
    }
}
