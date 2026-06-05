# DIM 06 – Remmina Multi-Protokoll-Client: Tiefenanalyse

## Zusammenfassung

Dieses Dokument analysiert den Remmina Remote Desktop Client (GitLab: `Remmina/Remmina`) als Referenzarchitektur fur einen multi-protokollfahigen Remote-Desktop-Viewer. Remmina ist ein in C/GTK geschriebener Open-Source-Client, der durch ein ausgeklugeltes Plugin-System mehrere Protokolle (RDP, VNC, SSH, SPICE, X2Go, HTTP, EXEC) in einer einheitlichen UI unterstutzt.

**Repository:** https://gitlab.com/Remmina/Remmina  
**Mirror:** https://github.com/FreeRDP/Remmina  
**Lizenz:** GPL-2.0+  
**Sprache:** C (GTK3)  

---

## Inhaltsverzeichnis

1. [Plugin-System-Architektur](#1-plugin-system-architektur)
2. [Plugin-API-Definition](#2-plugin-api-definition)
3. [Multi-Protokoll-Handler](#3-multi-protokoll-handler)
4. [Protocol-Abstraction-Layer](#4-protocol-abstraction-layer)
5. [Connection-Manager](#5-connection-manager)
6. [GTK-UI-Architektur](#6-gtk-ui-architektur)
7. [Feature-Plugins](#7-feature-plugins)
8. [Architektur-Muster fur ClawViewer](#8-architektur-muster-fur-clawviewer)

---

## 1. Plugin-System-Architektur

### 1.1 Uberblick

Remmina verwendet ein **dynamisches Plugin-System** basierend auf GModule (GLib's Wrapper fur dlopen). Plugins werden zur Laufzeit aus einem konfigurierbaren Verzeichnis geladen und uber eine zentrale Plugin-API in die Anwendung integriert.

**Quelle:** [^196^] `src/include/remmina/plugin.h`  
**Quelle:** [^194^] `src/remmina_plugin_manager.c`

### 1.2 Plugin-Typen

Remmina definiert **7 Plugin-Typen** uber das Enum `RemminaPluginType`:

```c
typedef enum {
    REMMINA_PLUGIN_TYPE_PROTOCOL    = 0,   // Protokoll-Handler (RDP, VNC, SSH, ...)
    REMMINA_PLUGIN_TYPE_ENTRY       = 1,   // Entry-Point Plugins
    REMMINA_PLUGIN_TYPE_FILE        = 2,   // Datei-Import/Export (z.B. .rdp Dateien)
    REMMINA_PLUGIN_TYPE_TOOL        = 3,   // Werkzeug-Plugins
    REMMINA_PLUGIN_TYPE_PREF        = 4,   // Praferenz-Plugins
    REMMINA_PLUGIN_TYPE_SECRET      = 5,   // Secret-Speicher (Keyring, etc.)
    REMMINA_PLUGIN_TYPE_LANGUAGE_WRAPPER = 6,  // Sprach-Wrapper
} RemminaPluginType;
```

**Claim:** Remmina unterstutzt 7 unterschiedliche Plugin-Kategorien, wobei Protocol-Plugins die zentralste Kategorie sind.  
**Source:** [^196^] `src/include/remmina/plugin.h`, Zeilen 58-66

### 1.3 Plugin-Loading-Mechanismus

Der Plugin-Manager verwendet `GModule` fur das dynamische Laden:

**Datei:** `src/remmina_plugin_manager.c` (48.85 KiB)

```c
static GPtrArray* remmina_plugin_table = NULL;
GPtrArray *loaded_plugins = NULL;
```

**Claim:** Plugins werden als Shared Libraries (.so-Dateien) geladen und in einem `GPtrArray` verwaltet.  
**Source:** [^194^] `src/remmina_plugin_manager.c`, Zeilen 64-65

**Plugin-Verzeichnisstruktur:**
- System-Plugins: `/usr/lib/remmina/plugins/`
- Benutzer-Plugins: `~/.config/remmina/plugins/`
- Plugin-Dateien: `remmina-plugin-<name>.so`

**Quelle:** [^193^] `plugins/README.md`

### 1.4 Plugin-Entry-Point

Jedes Plugin exportiert eine Entry-Funktion, die von Remmina aufgerufen wird:

```c
typedef gboolean (*RemminaPluginEntryFunc) (RemminaPluginService *service);
```

**Claim:** Jedes Plugin muss eine `remmina_plugin_entry()`-Funktion exportieren, die einen Zeiger auf `RemminaPluginService` erhalt.  
**Source:** [^196^] `src/include/remmina/plugin.h`, Zeile 276

### 1.5 Plugin-Manager-API

**Datei:** `src/remmina_plugin_manager.h`

```c
void remmina_plugin_manager_init(void);
RemminaPlugin *remmina_plugin_manager_get_plugin(RemminaPluginType type, const gchar *name);
gboolean remmina_plugin_manager_query_feature_by_type(RemminaPluginType ptype, const gchar *name, RemminaProtocolFeatureType ftype);
void remmina_plugin_manager_for_each_plugin(RemminaPluginType type, RemminaPluginFunc func, gpointer data);
RemminaFilePlugin *remmina_plugin_manager_get_import_file_handler(const gchar *file);
RemminaFilePlugin *remmina_plugin_manager_get_export_file_handler(RemminaFile *remminafile);
RemminaSecretPlugin *remmina_plugin_manager_get_secret_plugin(void);
```

**Claim:** Der Plugin-Manager bietet eine vollstandige API fur Plugin-Abfrage, Feature-Detection und Iteration.  
**Source:** [^197^] `src/remmina_plugin_manager.h`

---

## 2. Plugin-API-Definition

### 2.1 Protocol-Plugin-Struktur

Die zentrale Struktur fur alle Protokoll-Plugins ist `RemminaProtocolPlugin`:

**Datei:** `src/include/remmina/plugin.h`

```c
typedef struct _RemminaProtocolPlugin {
    RemminaPluginType           type;           // Muss REMMINA_PLUGIN_TYPE_PROTOCOL sein
    const gchar *               name;           // Plugin-Name (z.B. "RDP")
    const gchar *               description;    // Beschreibung
    const gchar *               domain;         // Translation domain
    const gchar *               version;        // Plugin-Version
    
    const gchar *               icon_name;      // Icon fur Protokoll
    const gchar *               icon_name_ssh;  // Icon fur SSH-Modus
    const RemminaProtocolSetting *basic_settings;      // Basis-Einstellungen
    const RemminaProtocolSetting *advanced_settings;   // Erweiterte Einstellungen
    RemminaProtocolSSHSetting   ssh_setting;    // SSH-Tunnel-Modus
    const RemminaProtocolFeature *features;     // Unterstutzte Features

    // Verbindungs-Lebenszyklus
    void (*init)(RemminaProtocolWidget *gp);
    gboolean (*open_connection)(RemminaProtocolWidget *gp);
    gboolean (*close_connection)(RemminaProtocolWidget *gp);
    
    // Feature-Handling
    gboolean (*query_feature)(RemminaProtocolWidget *gp, const RemminaProtocolFeature *feature);
    void (*call_feature)(RemminaProtocolWidget *gp, const RemminaProtocolFeature *feature);
    
    // Zusatzliche Funktionen
    void (*send_keystrokes)(RemminaProtocolWidget *gp, const guint keystrokes[], const gint keylen);
    gboolean (*get_plugin_screenshot)(RemminaProtocolWidget *gp, RemminaPluginScreenshotData *rpsd);
    gboolean (*map_event)(RemminaProtocolWidget *gp);
    gboolean (*unmap_event)(RemminaProtocolWidget *gp);
} RemminaProtocolPlugin;
```

**Claim:** Jedes Protocol-Plugin implementiert eine klar definierte Struktur mit Lebenszyklus-Callbacks (init, open_connection, close_connection) und Feature-Handling.  
**Source:** [^196^] `src/include/remmina/plugin.h`, Zeilen 93-117

### 2.2 Einstellungs-Typen

**Datei:** `src/include/remmina/types.h`

```c
typedef enum {
    REMMINA_PROTOCOL_SETTING_TYPE_END,
    REMMINA_PROTOCOL_SETTING_TYPE_SERVER,
    REMMINA_PROTOCOL_SETTING_TYPE_PASSWORD,
    REMMINA_PROTOCOL_SETTING_TYPE_RESOLUTION,
    REMMINA_PROTOCOL_SETTING_TYPE_ASSISTANCE,
    REMMINA_PROTOCOL_SETTING_TYPE_KEYMAP,
    REMMINA_PROTOCOL_SETTING_TYPE_TEXT,
    REMMINA_PROTOCOL_SETTING_TYPE_TEXTAREA,
    REMMINA_PROTOCOL_SETTING_TYPE_SELECT,
    REMMINA_PROTOCOL_SETTING_TYPE_COMBO,
    REMMINA_PROTOCOL_SETTING_TYPE_CHECK,
    REMMINA_PROTOCOL_SETTING_TYPE_FILE,
    REMMINA_PROTOCOL_SETTING_TYPE_FOLDER,
    REMMINA_PROTOCOL_SETTING_TYPE_INT,
    REMMINA_PROTOCOL_SETTING_TYPE_DOUBLE
} RemminaProtocolSettingType;
```

**Claim:** Remmina definiert 16 verschiedene UI-Setting-Typen, die automatisch in GTK-Widgets umgewandelt werden.  
**Source:** [^201^] `src/include/remmina/types.h`, Zeilen 56-76

### 2.3 Feature-Typen

```c
typedef enum {
    REMMINA_PROTOCOL_FEATURE_TYPE_END,
    REMMINA_PROTOCOL_FEATURE_TYPE_PREF,         // Praferenz-Feature
    REMMINA_PROTOCOL_FEATURE_TYPE_TOOL,         // Toolbar-Werkzeug
    REMMINA_PROTOCOL_FEATURE_TYPE_UNFOCUS,      // Unfocus-Verhalten
    REMMINA_PROTOCOL_FEATURE_TYPE_SCALE,        // Skalierung
    REMMINA_PROTOCOL_FEATURE_TYPE_DYNRESUPDATE, // Dynamische Auflosung
    REMMINA_PROTOCOL_FEATURE_TYPE_MULTIMON,     // Multi-Monitor
    REMMINA_PROTOCOL_FEATURE_TYPE_GTKSOCKET,    // GtkSocket-Unterstutzung
    REMMINA_PROTOCOL_FEATURE_TYPE_VIEWONLY,     // Nur-Anzeigen-Modus
} RemminaProtocolFeatureType;
```

**Source:** [^201^] `src/include/remmina/types.h`, Zeilen 40-52

### 2.4 Plugin Service (Callback-API)

**Datei:** `src/include/remmina/plugin.h` (270+ Zeilen)

Das `RemminaPluginService`-Struct enthalt uber 100 Funktionszeiger, die Plugins den Zugriff auf Remmina-Core-Funktionalitat ermoglichen:

```c
typedef struct _RemminaPluginService {
    // Plugin-Registrierung
    gboolean (*register_plugin)(RemminaPlugin *plugin);
    
    // Groessen-Management
    gint (*protocol_plugin_get_width)(RemminaProtocolWidget *gp);
    void (*protocol_plugin_set_width)(RemminaProtocolWidget *gp, gint width);
    gint (*protocol_plugin_get_height)(RemminaProtocolWidget *gp);
    void (*protocol_plugin_set_height)(RemminaProtocolWidget *gp, gint height);
    
    // Fehler-Handling
    gboolean (*protocol_plugin_has_error)(RemminaProtocolWidget *gp);
    void (*protocol_plugin_set_error)(RemminaProtocolWidget *gp, const gchar *fmt, ...);
    gboolean (*protocol_plugin_is_closed)(RemminaProtocolWidget *gp);
    
    // Datei-Zugriff (Verbindungsprofil)
    RemminaFile * (*protocol_plugin_get_file)(RemminaProtocolWidget * gp);
    
    // Signal-Emission
    void (*protocol_plugin_emit_signal)(RemminaProtocolWidget *gp, const gchar *signal_name);
    void (*protocol_plugin_signal_connection_closed)(RemminaProtocolWidget *gp);
    void (*protocol_plugin_signal_connection_opened)(RemminaProtocolWidget *gp);
    
    // Tunnel-Unterstutzung
    gchar * (*protocol_plugin_start_direct_tunnel)(RemminaProtocolWidget *gp, gint default_port, gboolean port_plus);
    gboolean (*protocol_plugin_start_reverse_tunnel)(RemminaProtocolWidget *gp, gint local_port);
    
    // Authentifizierungs-UI
    gint (*protocol_plugin_init_auth)(RemminaProtocolWidget *gp, RemminaMessagePanelFlags pflags, const gchar *title, ...);
    gchar * (*protocol_plugin_init_get_username)(RemminaProtocolWidget * gp);
    gchar * (*protocol_plugin_init_get_password)(RemminaProtocolWidget * gp);
    gchar * (*protocol_plugin_init_get_domain)(RemminaProtocolWidget * gp);
    
    // Datei-Operationen
    gchar * (*file_get_user_datadir)(void);
    RemminaFile * (*file_new)(void);
    void (*file_set_string)(RemminaFile *remminafile, const gchar *setting, const gchar *value);
    const gchar * (*file_get_string)(RemminaFile * remminafile, const gchar *setting);
    void (*file_set_int)(RemminaFile *remminafile, const gchar *setting, gint value);
    gint (*file_get_int)(RemminaFile *remminafile, const gchar *setting, gint default_value);
    
    // Praferenzen
    void (*pref_set_value)(const gchar *key, const gchar *value);
    gchar * (*pref_get_value)(const gchar * key);
    
    // Logging
    void (*_remmina_info)(const gchar *fmt, ...);
    void (*_remmina_debug)(const gchar *func, const gchar *fmt, ...);
    void (*_remmina_error)(const gchar *func, const gchar *fmt, ...);
    
    // Fenster-Management
    GtkWidget * (*open_connection)(RemminaFile * remminafile, GCallback disconnect_cb, gpointer data, guint *handler);
    GtkWindow * (*get_window)(void);
    
    // ... (uber 100 Funktionen insgesamt)
} RemminaPluginService;
```

**Claim:** Remmina bietet ein umfassendes "Plugin Service"-Interface mit uber 100 Funktionszeigern, das Plugins kontrollierten Zugriff auf alle Core-Funktionalitaten ermoglicht, ohne direkte Abhangigkeiten zu erzeugen.  
**Source:** [^196^] `src/include/remmina/plugin.h`, Zeilen 141-276

---

## 3. Multi-Protokoll-Handler

### 3.1 RDP-Plugin

**Verzeichnis:** `plugins/rdp/`  
**Hauptdatei:** `rdp_plugin.c` (rdp_plugin.h, rdp_event.c, rdp_graphics.c, rdp_cliprdr.c, etc.)

**Plugin-Registrierung:**

```c
static RemminaProtocolPlugin remmina_rdp_plugin = {
    REMMINA_PLUGIN_TYPE_PROTOCOL,
    "RDP",
    N_("RDP - Remote Desktop Protocol"),
    GETTEXT_PACKAGE,
    VERSION,
    "org.remmina.Remmina-rdp-symbolic",
    "org.remmina.Remmina-rdp-ssh-symbolic",
    remmina_rdp_basic_settings,
    remmina_rdp_advanced_settings,
    REMMINA_PROTOCOL_SSH_SETTING_TUNNEL,
    remmina_rdp_features,
    remmina_rdp_init,
    remmina_rdp_open_connection,
    remmina_rdp_close_connection,
    remmina_rdp_query_feature,
    remmina_rdp_call_feature,
    remmina_rdp_send_keystrokes,
    remmina_rdp_get_plugin_screenshot,
    remmina_rdp_map_event,
    remmina_rdp_unmap_event
};
```

**RDP-Plugin-Features:**
```c
#define REMMINA_RDP_FEATURE_TOOL_REFRESH         1
#define REMMINA_RDP_FEATURE_SCALE                2
#define REMMINA_RDP_FEATURE_UNFOCUS              3
#define REMMINA_RDP_FEATURE_TOOL_SENDCTRLALTDEL  4
#define REMMINA_RDP_FEATURE_DYNRESUPDATE         5
#define REMMINA_RDP_FEATURE_MULTIMON             6
#define REMMINA_RDP_FEATURE_VIEWONLY             7
```

**Claim:** Das RDP-Plugin basiert auf FreeRDP (libfreerdp) und implementiert den vollstandigen RDP-Protokoll-Stack inkl. Grafik, Audio, Clipboard, Datei-Transfer und Multi-Monitor.  
**Source:** [^201^] `plugins/rdp/rdp_plugin.c`

**RDP-Plugin-Einstellungen (basic_settings):**
- Server
- Username
- Password
- Domain
- Resolution
- Color depth
- Share folder
- Sound (Audio-Redirection)
- Security
- Gateway settings
- Client name
- Startup program
- Printer sharing
- Smartcard sharing
- Disable clipboard sync

**Source:** [^212^] https://remmina.org/remmina-features/

### 3.2 VNC-Plugin

**Verzeichnis:** `plugins/vnc/`  
**Hauptdatei:** `vnc_plugin.c`

**Plugin-Registrierung:**
```c
static RemminaProtocolPlugin remmina_plugin_vnc = {
    REMMINA_PLUGIN_TYPE_PROTOCOL,
    "VNC",
    N_("VNC - VNC viewer"),
    GETTEXT_PACKAGE,
    VERSION,
    ...
    remmina_plugin_vnc_basic_settings,
    remmina_plugin_vnc_advanced_settings,
    REMMINA_PROTOCOL_SSH_SETTING_TUNNEL,
    remmina_plugin_vnc_features,
    remmina_plugin_vnc_init,
    remmina_plugin_vnc_open_connection,
    remmina_plugin_vnc_close_connection,
    ...
};
```

**VNC-Plugin-Features:**
```c
#define REMMINA_PLUGIN_VNC_FEATURE_PREF_QUALITY            1
#define REMMINA_PLUGIN_VNC_FEATURE_VIEWONLY                2
#define REMMINA_PLUGIN_VNC_FEATURE_PREF_DISABLESERVERINPUT 3
#define REMMINA_PLUGIN_VNC_FEATURE_TOOL_REFRESH            4
#define REMMINA_PLUGIN_VNC_FEATURE_TOOL_CHAT               5
#define REMMINA_PLUGIN_VNC_FEATURE_SCALE                   6
#define REMMINA_PLUGIN_VNC_FEATURE_UNFOCUS                 7
#define REMMINA_PLUGIN_VNC_FEATURE_TOOL_SENDCTRLALTDEL     8
#define REMMINA_PLUGIN_VNC_FEATURE_PREF_COLOR              9
#define REMMINA_PLUGIN_VNC_FEATURE_DYNRESUPDATE           10
```

**Claim:** Das VNC-Plugin basiert auf LibVNCClient (libvncclient) und unterstutzt verschiedene Encodings (Tight, ZRLE, Hextile, etc.), Quality-Levels und Color Depths.  
**Source:** [^200^] `plugins/vnc/vnc_plugin.c`

**VNC-Plugin-Schlussel-Eigenschaften:**
- Verwendet `rfbClient` aus libvncclient
- Unterstutzt verschiedene Authentifizierungsmethoden (None, VNCAuth, MSLogon, etc.)
- Clipboard-Integration via `rfbClientCutText`
- Cursor-Shape-Support
- Chat-Funktion (UltraVNC)
- Listener-Modus (Reverse VNC)
- Qualitatsstufen: 0 (Poor/fastest) bis 9 (Best/slowest)

### 3.3 SSH-Plugin

**Verzeichnis:** `src/` (im Hauptprogramm integriert)  
**Hauptdatei:** `remmina_ssh_plugin.c`

**Plugin-Registrierung:**
```c
static RemminaProtocolPlugin remmina_plugin_ssh = {
    REMMINA_PLUGIN_TYPE_PROTOCOL,
    "SSH",
    N_("SSH - Secure Shell"),
    GETTEXT_PACKAGE,
    VERSION,
    ...
    remmina_plugin_ssh_basic_settings,
    remmina_plugin_ssh_advanced_settings,
    REMMINA_PROTOCOL_SSH_SETTING_SSH,
    remmina_plugin_ssh_features,
    remmina_plugin_ssh_init,
    remmina_plugin_ssh_open_connection,
    remmina_plugin_ssh_close_connection,
    ...
};
```

**SSH-Plugin-Features:**
```c
#define REMMINA_PLUGIN_SSH_FEATURE_TOOL_COPY        1
#define REMMINA_PLUGIN_SSH_FEATURE_TOOL_PASTE       2
#define REMMINA_PLUGIN_SSH_FEATURE_TOOL_SELECT_ALL  3
#define REMMINA_PLUGIN_SSH_FEATURE_TOOL_INCREASE_FONT 4
#define REMMINA_PLUGIN_SSH_FEATURE_TOOL_DECREASE_FONT 5
#define REMMINA_PLUGIN_SSH_FEATURE_TOOL_SEARCH      6
```

**Claim:** Das SSH-Plugin verwendet libssh und libvte (Virtual Terminal Emulator) fur eine vollwertige Terminal-Erfahrung mit Farbpaletten, Font-Skalierung, Suchfunktion und Clipboard-Integration.  
**Source:** [^205^] `src/remmina_ssh_plugin.c`

**SSH-Plugin-Schlussel-Eigenschaften:**
- Basierend auf VTE (Virtual Terminal Emulator)
- Unterstutzt 6 eingebaute Farbpaletten (Linux, Tango, Gruvbox, Solarized Dark/Light, XTerm)
- Benutzerdefinierte Farbthemen via `.colors`-Dateien
- Font-Skalierung (inkrementell)
- Session-Speicherung
- Suchfunktion mit Regex
- Audible Bell
- Kopieren/Einfugen
- SFTP-Transfer-Integration

---

## 4. Protocol-Abstraction-Layer

### 4.1 RemminaProtocolWidget

**Datei:** `src/remmina_protocol_widget.h` und `src/remmina_protocol_widget.c`

`RemminaProtocolWidget` ist das zentrale GTK-Widget, das als **Container und Abstraktionsschicht** fur alle Protokoll-Plugins dient:

```c
struct _RemminaProtocolWidget {
    GtkEventBox              event_box;
    RemminaConnectionObject *cnnobj;
    RemminaProtocolWidgetPriv *priv;
    RemminaProtocolPlugin   *plugin;
};
```

**Claim:** `RemminaProtocolWidget` ist die zentrale Abstraktionsschicht, die alle Protokolle einheitlich behandelt. Es erbt von `GtkEventBox` und enthalt einen Verweis auf das geladene Protocol-Plugin.  
**Source:** [^202^] `src/remmina_protocol_widget.h`, Zeilen 52-57

### 4.2 Lebenszyklus-Management

Das Protocol-Widget verwaltet den Verbindungs-Lebenszyklus:

```c
// Initialisierung
GtkWidget *remmina_protocol_widget_new(void);
void remmina_protocol_widget_setup(RemminaProtocolWidget *gp, RemminaFile *remminafile, RemminaConnectionObject *cnnobj);

// Verbindungs-Steuerung
void remmina_protocol_widget_open_connection(RemminaProtocolWidget *gp);
void remmina_protocol_widget_close_connection(RemminaProtocolWidget *gp);

// Signal-Handling
void remmina_protocol_widget_signal_connection_closed(RemminaProtocolWidget *gp);
void remmina_protocol_widget_signal_connection_opened(RemminaProtocolWidget *gp);
```

### 4.3 Feature-Abfrage und -Aufruf

```c
const RemminaProtocolFeature *remmina_protocol_widget_get_features(RemminaProtocolWidget *gp);
gboolean remmina_protocol_widget_query_feature_by_type(RemminaProtocolWidget *gp, RemminaProtocolFeatureType type);
gboolean remmina_protocol_widget_query_feature_by_ref(RemminaProtocolWidget *gp, const RemminaProtocolFeature *feature);
void remmina_protocol_widget_call_feature_by_type(RemminaProtocolWidget *gp, RemminaProtocolFeatureType type, gint id);
void remmina_protocol_widget_call_feature_by_ref(RemminaProtocolWidget *gp, const RemminaProtocolFeature *feature);
```

### 4.4 Groessen- und Skalierungs-Management

```c
gint remmina_protocol_widget_get_width(RemminaProtocolWidget *gp);
void remmina_protocol_widget_set_width(RemminaProtocolWidget *gp, gint width);
gint remmina_protocol_widget_get_height(RemminaProtocolWidget *gp);
void remmina_protocol_widget_set_height(RemminaProtocolWidget *gp, gint height);

// Skalierungs-Modi
typedef enum {
    REMMINA_PROTOCOL_WIDGET_SCALE_MODE_NONE     = 0,  // Keine Skalierung
    REMMINA_PROTOCOL_WIDGET_SCALE_MODE_SCALED   = 1,  // Statische Skalierung
    REMMINA_PROTOCOL_WIDGET_SCALE_MODE_DYNRES   = 2,  // Dynamische Auflosung
} RemminaScaleMode;
```

**Source:** [^202^] `src/remmina_protocol_widget.h`

### 4.5 Trennung von UI und Protokoll

**Claim:** Remmina trennt UI und Protokoll-Handler durch das "Plugin Service"-Pattern. Protokoll-Plugins haben keinen direkten Zugriff auf die UI; alle UI-Interaktionen laufen uber die `RemminaPluginService`-Funktionszeiger.  
**Source:** [^196^] `src/include/remmina/plugin.h`, `RemminaPluginService`-Struct

---

## 5. Connection-Manager

### 5.1 Verbindungsprofile (RemminaFile)

**Datei:** `src/remmina_file.h`

```c
struct _RemminaFile {
    gchar *         filename;       // Dateipfad des Profils
    gchar *         statefile;      // Status-Datei
    GHashTable *    settings;       // Profil-Einstellungen (Key-Value)
    GHashTable *    states;         // Laufzeit-Status
    GHashTable *    spsettings;     // Secret-Einstellungen
    gboolean        prevent_saving; // Speicherung verhindern
};
```

**Claim:** Verbindungsprofile werden als `RemminaFile`-Objekte mit GHashTable-basierten Key-Value-Stores verwaltet. Die Serialisierung erfolgt in `.remmina`-Dateien im INI-Format.  
**Source:** [^202^] `src/remmina_file.h`, Zeilen 49-57

### 5.2 Profil-Speicherung

**Speicherorte:**
- Benutzer-Profile: `~/.local/share/remmina/` oder `~/.config/remmina/`
- Globale Defaults: `/etc/xdg/remmina/`
- Dateiendung: `.remmina`

**Beispiel einer .remmina-Datei:**
```ini
[remmina]
name=Windows Server
protocol=RDP
server=192.168.1.100
username=admin
domain=COMPANY
colordepth=32
resolution_mode=2
sharefolder=/home/user/shared
enableaudio=1
quality=0
security=ntlm
disablepasswordstoring=0
precommand=
postcommand=
viewmode=1
scale=1
```

**Source:** [^171^] https://wiki.archlinux.org/title/Remmina

### 5.3 Profil-API

```c
// Erstellen und Laden
RemminaFile *remmina_file_new(void);
RemminaFile *remmina_file_load(const gchar *filename);

// Einstellungen setzen/lesen
void remmina_file_set_string(RemminaFile *remminafile, const gchar *setting, const gchar *value);
const gchar *remmina_file_get_string(RemminaFile *remminafile, const gchar *setting);
void remmina_file_set_int(RemminaFile *remminafile, const gchar *setting, gint value);
gint remmina_file_get_int(RemminaFile *remminafile, const gchar *setting, gint default_value);

// Speichern und Freigeben
void remmina_file_save(RemminaFile *remminafile);
void remmina_file_free(RemminaFile *remminafile);

// Duplizieren und Manipulieren
RemminaFile *remmina_file_dup(RemminaFile *remminafile);
RemminaFile *remmina_file_dup_temp_protocol(RemminaFile *remminafile, const gchar *new_protocol);
```

### 5.4 URI-basierte Quick-Connects

Remmina unterstutzt URI-basierte Quick-Connects:
- `rdp://user@example.com`
- `rdp://DOMAIN\\user@example.com`
- `vnc://user@example.com`
- `vnc://example.com?VncUsername=user`
- `ssh://user@example.com`
- `spice://example.com`

**Source:** [^171^] https://wiki.archlinux.org/title/Remmina

---

## 6. GTK-UI-Architektur

### 6.1 Uberblick

Remmina verwendet **GTK3** als UI-Toolkit. Die Architektur besteht aus:

1. **Main Window** (`remmina_main.c`) - Hauptfenster mit Verbindungsliste
2. **Connection Window** (`rcw.c`) - Fenster fur aktive Verbindungen (Tabs)
3. **File Editor** (`remmina_file_editor.c`) - Verbindungsprofil-Editor
4. **Preferences Dialog** (`remmina_pref_dialog.c`) - Einstellungen

### 6.2 Remmina Connection Window (RCW)

**Datei:** `src/rcw.h` und `src/rcw.c`

```c
typedef struct _RemminaConnectionWindow {
    GtkWindow                        window;
    RemminaConnectionWindowPriv *    priv;
} RemminaConnectionWindow;
```

**Claim:** Die `RemminaConnectionWindow` ist das Hauptfenster fur aktive Verbindungen. Sie unterstutzt einen Tab-basierten Ansatz, in dem mehrere Verbindungen gleichzeitig angezeigt werden konnen.  
**Source:** [^203^] `src/rcw.h`, Zeilen 50-53

### 6.3 Fenster-Modi

Die RCW unterstutzt verschiedene View-Modes:
```c
typedef enum {
    RES_INVALID                     = -1,
    RES_USE_CUSTOM                  = 0,   // Benutzerdefinierte Auflosung
    RES_USE_CLIENT                  = 1,   // Client-Auflosung
    RES_USE_INITIAL_WINDOW_SIZE     = 2,   // Initiale Fenstergrosse
} RemminaProtocolWidgetResolutionMode;
```

### 6.4 UI-Registration durch Plugins

```c
// Aus RemminaPluginService
void (*ui_register)(GtkWidget *widget);

// Fenster-Offnung
GtkWidget * (*open_connection)(RemminaFile * remminafile, GCallback disconnect_cb, gpointer data, guint *handler);
GtkWidget * (*rcw_open_from_file_full)(RemminaFile *remminafile, GCallback disconnect_cb, gpointer data, guint *handler);
```

### 6.5 Message Panel API

Plugins konnen UI-Panels fur Authentifizierung und Status anzeigen:

```c
gint remmina_protocol_widget_panel_auth(RemminaProtocolWidget *gp, RemminaMessagePanelFlags pflags, const gchar *title, ...);
gint remmina_protocol_widget_panel_new_certificate(RemminaProtocolWidget *gp, const gchar *subject, const gchar *issuer, const gchar *fingerprint);
gint remmina_protocol_widget_panel_changed_certificate(RemminaProtocolWidget *gp, const gchar *subject, const gchar *issuer, const gchar *new_fingerprint, const gchar *old_fingerprint);
void remmina_protocol_widget_panel_show(RemminaProtocolWidget *gp);
void remmina_protocol_widget_panel_hide(RemminaProtocolWidget *gp);
```

### 6.6 Toolbar und Features

Jedes Protokoll-Plugin definiert seine eigenen Toolbar-Features, die von der RCW dynamisch erstellt werden:

```c
// Feature-Definition im Plugin
static const RemminaProtocolFeature remmina_rdp_features[] = {
    { REMMINA_PROTOCOL_FEATURE_TYPE_TOOL, REMMINA_RDP_FEATURE_TOOL_REFRESH, N_("Refresh"), NULL, NULL },
    { REMMINA_PROTOCOL_FEATURE_TYPE_SCALE, REMMINA_RDP_FEATURE_SCALE, NULL, NULL, NULL },
    { REMMINA_PROTOCOL_FEATURE_TYPE_DYNRESUPDATE, REMMINA_RDP_FEATURE_DYNRESUPDATE, NULL, NULL, NULL },
    { REMMINA_PROTOCOL_FEATURE_TYPE_TOOL, REMMINA_RDP_FEATURE_TOOL_SENDCTRLALTDEL, N_("Send Ctrl+Alt+Del"), NULL, NULL },
    { REMMINA_PROTOCOL_FEATURE_TYPE_MULTIMON, REMMINA_RDP_FEATURE_MULTIMON, NULL, NULL, NULL },
    { REMMINA_PROTOCOL_FEATURE_TYPE_END, 0, NULL, NULL, NULL }
};
```

---

## 7. Feature-Plugins

### 7.1 Clipboard-Integration

**RDP-Clipboard:**
- Implementiert in `plugins/rdp/rdp_cliprdr.c`
- Verwendet den RDP CLIPRDR-Kanal
- Unterstutzt Text, Bilder und Dateien
- Bidirektionale Synchronisation

**VNC-Clipboard:**
- Implementiert in `plugins/vnc/vnc_plugin.c` (Funktion `remmina_plugin_vnc_rfb_cuttext`)
- Verwendet VNC ClientCutText-Protokoll
- Konvertierung zwischen ISO-8859-1 und UTF-8

### 7.2 Audio-Redirection

**RDP-Audio:**
- Verwendet FreeRDP's Audio-Kanale (rdpsnd, audin)
- Unterstutzt lokale und Remote-Ausgabe
- Konfigurierbare Qualitatsstufen
- Mikrofon-Umleitung

**Konfiguration:**
```
Audio output mode: Local/Remote/None
Redirect local microphone: sys:pulse,format:1,quality:high
```

**Source:** [^200^] GitLab Issue #2417

### 7.3 Datei-Transfer

**RDP-Datei-Transfer:**
- Verwendet RDP Drive Redirection (RDPDR)
- Lokale Ordner werden als Netzlaufwerke im RDP-Session bereitgestellt
- Kein explizites "Transfer" notig - Dateien sind direkt zuganglich

**SFTP-Datei-Transfer:**
- Separates SFTP-Plugin (`src/remmina_sftp_plugin.c`)
- Eingebetteter Datei-Manager mit zwei Paneelen
- Drag-and-Drop-Unterstutzung
- Integration mit SSH-Verbindungen

**Source:** [^195^] https://www.tecmint.com/remmina-remote-desktop-sharing-and-ssh-client/

### 7.4 Secret-Plugin

**Verzeichnis:** `plugins/secret/`

Das Secret-Plugin speichert Passworter sicher:
- Verwendet libsecret / GNOME Keyring
- KDE Wallet Unterstutzung (separates Plugin)
- Verschlusselte Speicherung von Credentials

```c
typedef struct _RemminaSecretPlugin {
    RemminaPluginType   type;
    ...
    int                 init_order;
    gboolean (*init)(struct _RemminaSecretPlugin* instance);
    gboolean (*is_service_available)(struct _RemminaSecretPlugin* instance);
    void (*store_password)(struct _RemminaSecretPlugin* instance, RemminaFile *remminafile, const gchar *key, const gchar *password);
    gchar * (*get_password)(struct _RemminaSecretPlugin* instance, RemminaFile * remminafile, const gchar *key);
    void (*delete_password)(struct _RemminaSecretPlugin* instance, RemminaFile *remminafile, const gchar *key);
} RemminaSecretPlugin;
```

---

## 8. Architektur-Muster fur ClawViewer

### 8.1 Ubernehmbare Muster

#### 8.1.1 Plugin-Service-Architektur

**Pattern:** Definiere ein zentrales Service-Struct mit Funktionszeigern, das allen Plugins als einzige Schnittstelle zum Core dient.

```c
// ClawViewer-Analogon:
typedef struct _ClawPluginService {
    gboolean (*register_plugin)(ClawPlugin *plugin);
    
    // Canvas/Rendering
    gint (*canvas_get_width)(ClawProtocolWidget *pw);
    void (*canvas_set_width)(ClawProtocolWidget *pw, gint width);
    gint (*canvas_get_height)(ClawProtocolWidget *pw);
    void (*canvas_set_height)(ClawProtocolWidget *pw, gint height);
    void (*canvas_request_redraw)(ClawProtocolWidget *pw, gint x, gint y, gint w, gint h);
    
    // Eingabe
    void (*input_register_handler)(ClawProtocolWidget *pw, ClawInputHandler *handler);
    
    // Profil/Settings
    ClawProfile * (*profile_get)(ClawProtocolWidget *pw);
    const gchar * (*profile_get_string)(ClawProfile *profile, const gchar *key);
    void (*profile_set_string)(ClawProfile *profile, const gchar *key, const gchar *value);
    
    // UI-Integration
    void (*ui_show_auth_dialog)(ClawProtocolWidget *pw, ClawAuthFlags flags, const gchar *title, ...);
    void (*ui_show_message)(ClawProtocolWidget *pw, const gchar *message);
    void (*ui_set_status)(ClawProtocolWidget *pw, const gchar *status);
    
    // Logging
    void (*log_debug)(const gchar *fmt, ...);
    void (*log_error)(const gchar *fmt, ...);
    
    // Tunnel
    gchar * (*tunnel_start_direct)(ClawProtocolWidget *pw, gint default_port);
    gchar * (*tunnel_start_reverse)(ClawProtocolWidget *pw, gint local_port);
} ClawPluginService;
```

#### 8.1.2 Protocol-Widget-Abstraktion

**Pattern:** Erstelle ein generisches Container-Widget, das als Host fur alle Protokoll-Implementierungen dient.

```c
// ClawViewer-Analogon:
typedef struct _ClawProtocolWidget {
    GtkEventBox           parent;
    ClawConnectionObject *cnnobj;
    ClawProtocolWidgetPriv *priv;
    ClawProtocolPlugin   *plugin;  // Aktives Protokoll-Plugin
    GtkWidget            *drawing_area;
} ClawProtocolWidget;
```

#### 8.1.3 GHashTable-basierte Settings

**Pattern:** Verwende GHashTable fur flexible Key-Value-Settings in Verbindungsprofilen.

```c
typedef struct _ClawProfile {
    gchar *       filename;
    GHashTable *  settings;   // Alle Profileinstellungen
    GHashTable *  secrets;    // Verschlusselte Werte
    gchar *       protocol;   // Protokoll-Name
} ClawProfile;
```

#### 8.1.4 Feature-Registrierung

**Pattern:** Protokolle deklarieren ihre Features statisch; die UI erstellt dynamisch die passenden Controls.

```c
typedef struct _ClawProtocolFeature {
    ClawFeatureType  type;    // TOOL, SCALE, DYNRES, etc.
    gint             id;      // Feature-ID
    const gchar *    label;   // Anzeigetext
    const gchar *    icon;    // Icon-Name
} ClawProtocolFeature;

// Beispiel-Features fur VNC:
static const ClawProtocolFeature vnc_features[] = {
    { CLAW_FEATURE_TYPE_TOOL, 1, "Refresh", "view-refresh" },
    { CLAW_FEATURE_TYPE_SCALE, 2, NULL, NULL },
    { CLAW_FEATURE_TYPE_DYNRES, 3, NULL, NULL },
    { CLAW_FEATURE_TYPE_END, 0, NULL, NULL }
};
```

#### 8.1.5 Plugin-Typ-System

**Pattern:** Verwende ein Enum-basiertes Typ-System fur verschiedene Plugin-Kategorien.

```c
typedef enum {
    CLAW_PLUGIN_TYPE_PROTOCOL,   // RDP, VNC, etc.
    CLAW_PLUGIN_TYPE_CODEC,      // Video-Codecs
    CLAW_PLUGIN_TYPE_TOOL,       // Werkzeuge
    CLAW_PLUGIN_TYPE_SECRET,     // Passwort-Verwaltung
    CLAW_PLUGIN_TYPE_THEME,      // UI-Themes
} ClawPluginType;
```

### 8.2 ClawViewer-spezifische Anpassungen

| Remmina-Muster | ClawViewer-Adaption |
|----------------|---------------------|
| GTK3 + GObject | GTK4 + GObject (modernisiert) |
| GdkDrawingArea | GtkDrawingArea mit GdkPaintable |
| GModule-Ladung | GModule-Ladung (gleich) |
| Cairo-Rendering | Cairo + OpenGL/Renderer-Abstraktion |
| FreeRDP/libvncclient | Rust- FFI-Wrapper fur Protokolle |
| INI-Profile | TOML-Profile |
| GHashTable-Settings | GHashTable-Settings (gleich) |
| VTE-Terminal | VTE-Terminal (gleich, oder wezterm-embed) |

### 8.3 Implementierungs-Roadmap

1. **Phase 1: Core-Framework**
   - Plugin-Manager mit GModule
   - Plugin-Service-API
   - Protocol-Widget-Abstraktion

2. **Phase 2: Protokoll-Plugins**
   - VNC-Plugin (libvncclient-Wrapper)
   - RDP-Plugin (FreeRDP-Wrapper)
   - SSH-Terminal-Plugin

3. **Phase 3: UI**
   - Main-Window mit Verbindungsliste
   - Connection-Window mit Tabs
   - Profile-Editor
   - Preferences

4. **Phase 4: Features**
   - Clipboard-Integration
   - Audio-Redirection
   - Datei-Transfer
   - Secret-Speicher

---

## A. Referenzen

### A.1 Quellcode-Dateien

| Datei | Beschreibung | Referenz |
|-------|-------------|----------|
| `src/include/remmina/plugin.h` | Plugin-API-Definition | [^196^] |
| `src/include/remmina/types.h` | Gemeinsame Typen und Enums | [^201^] |
| `src/remmina_plugin_manager.c` | Plugin-Manager-Implementierung | [^194^] |
| `src/remmina_plugin_manager.h` | Plugin-Manager-Header | [^197^] |
| `src/remmina_protocol_widget.c` | Protocol-Widget-Implementierung | [^194^] |
| `src/remmina_protocol_widget.h` | Protocol-Widget-Header | [^202^] |
| `src/remmina_file.h` | Verbindungsprofil-Definition | [^202^] |
| `src/rcw.h` | Remmina Connection Window | [^203^] |
| `src/remmina_ssh_plugin.c` | SSH-Plugin-Implementierung | [^205^] |
| `plugins/rdp/rdp_plugin.c` | RDP-Plugin-Implementierung | [^201^] |
| `plugins/rdp/rdp_plugin.h` | RDP-Plugin-Header | [^205^] |
| `plugins/vnc/vnc_plugin.c` | VNC-Plugin-Implementierung | [^200^] |
| `plugins/vnc/vnc_plugin.h` | VNC-Plugin-Header | [^200^] |
| `plugins/secret/` | Secret-Plugin-Verzeichnis | [^194^] |
| `plugins/tool_hello_world/` | Beispiel-Plugin | [^193^] |
| `plugins/common/` | Gemeinsamer Plugin-Code | [^193^] |
| `plugins/exec/` | EXEC-Plugin | [^194^] |
| `plugins/gvnc/` | GVNC-Plugin | [^194^] |
| `plugins/spice/` | SPICE-Plugin | [^194^] |
| `plugins/x2go/` | X2Go-Plugin | [^194^] |
| `plugins/www/` | WWW-Plugin | [^194^] |
| `src/remmina_main.c` | Hauptfenster-Implementierung | [^193^] |
| `src/remmina_file_editor.c` | Profil-Editor | [^193^] |
| `src/remmina_pref.c` | Praferenz-Verwaltung | [^193^] |
| `src/remmina_pref_dialog.c` | Praferenz-Dialog | [^193^] |

### A.2 Verzeichnisstruktur

```
Remmina/
├── src/                          # Hauptanwendung
│   ├── include/remmina/          # Offentliche Header
│   │   ├── plugin.h              # Plugin-API
│   │   ├── types.h               # Gemeinsame Typen
│   │   └── remmina_trace_calls.h
│   ├── remmina.c                 # main()-Funktion
│   ├── remmina_main.c/.h         # Hauptfenster
│   ├── rcw.c/.h                  # Connection Window
│   ├── remmina_protocol_widget.c/.h  # Protocol-Widget
│   ├── remmina_plugin_manager.c/.h   # Plugin-Manager
│   ├── remmina_file.c/.h         # Verbindungsprofil
│   ├── remmina_file_editor.c/.h  # Profil-Editor
│   ├── remmina_pref.c/.h         # Praferenzen
│   ├── remmina_pref_dialog.c/.h  # Praferenz-Dialog
│   ├── remmina_ssh_plugin.c/.h   # SSH-Plugin (built-in)
│   ├── remmina_sftp_plugin.c/.h  # SFTP-Plugin (built-in)
│   ├── remmina_ssh.c/.h          # SSH-Core
│   ├── remmina_log.c/.h          # Logging
│   ├── remmina_crypt.c/.h        # Verschlusselung
│   └── ...
├── plugins/                      # Protokoll-Plugins
│   ├── rdp/                      # RDP-Plugin
│   │   ├── rdp_plugin.c/.h
│   │   ├── rdp_event.c/.h
│   │   ├── rdp_graphics.c/.h
│   │   ├── rdp_cliprdr.c/.h
│   │   └── ...
│   ├── vnc/                      # VNC-Plugin
│   │   ├── vnc_plugin.c/.h
│   │   └── ...
│   ├── spice/                    # SPICE-Plugin
│   ├── x2go/                     # X2Go-Plugin
│   ├── exec/                     # EXEC-Plugin
│   ├── gvnc/                     # GVNC-Plugin
│   ├── secret/                   # Secret-Plugin
│   ├── kwallet/                  # KWallet-Plugin
│   ├── telepathy/                # Telepathy-Plugin
│   ├── python_wrapper/           # Python-Wrapper
│   ├── tool_hello_world/         # Beispiel-Plugin
│   └── common/                   # Gemeinsamer Code
├── cmake/                        # CMake-Module
├── data/                         # UI-Dateien, Icons, Themes
├── po/                           # Ubersetzungen
└── flatpak/, snap/              # Packaging
```

### A.3 Externe Referenzen

- **GitLab Repository:** https://gitlab.com/Remmina/Remmina [^186^]
- **GitHub Mirror:** https://github.com/FreeRDP/Remmina [^175^]
- **Offizielle Website:** https://remmina.org [^179^]
- **Features-Dokumentation:** https://remmina.org/remmina-features/ [^212^]
- **Arch Wiki:** https://wiki.archlinux.org/title/Remmina [^171^]

---

*Diese Analyse wurde am 2025-01-10 erstellt und basiert auf dem Remmina-Master-Branch (GitLab Commit e57fbb60f110b43b0161975afe798a835cae62e8 und umgebende Commits).*
