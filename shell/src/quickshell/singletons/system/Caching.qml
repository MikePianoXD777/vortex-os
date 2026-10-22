pragma Singleton
import QtQuick
import Quickshell

QtObject {
    id: root

    readonly property string serpantinumDir: Quickshell.env("SERPANTINUM_DIR")
    readonly property string qsDir: Quickshell.env("QS_DIR")
    readonly property string mainQml: Quickshell.env("MAIN_QML")

    readonly property string home: Quickshell.env("HOME")
    readonly property string xdgRuntimeDir: Quickshell.env("XDG_RUNTIME_DIR")

    readonly property string cacheDir: Quickshell.env("QS_CACHE_DIR") ? Quickshell.env("QS_CACHE_DIR") : (home + "/.cache/serpantinum")
    readonly property string stateDir: Quickshell.env("QS_STATE_DIR") ? Quickshell.env("QS_STATE_DIR") : (home + "/.local/state/serpantinum")
    readonly property string runDir: Quickshell.env("QS_RUN_DIR") ? Quickshell.env("QS_RUN_DIR") : ((xdgRuntimeDir !== "" ? xdgRuntimeDir : "/tmp") + "/serpantinum")
    readonly property string logDir: Quickshell.env("QS_LOG_DIR") ? Quickshell.env("QS_LOG_DIR") : (runDir + "/logs")

    // Vortex: cada carpeta se crea una sola vez por sesión. Antes cada llamada lanzaba
    // «mkdir -p» con execDetached (un doble fork del shell entero) y estas funciones se
    // llaman desde ~70 sitios, varias dentro de bindings que se reevalúan. El mkdir ya era
    // asíncrono, así que nadie dependía de que terminara antes de seguir.
    property var _createdDirs: ({})

    function ensureDir(path) {
        if (!path || root._createdDirs[path]) return;
        root._createdDirs[path] = true;
        Quickshell.execDetached(["mkdir", "-p", path]);
    }

    function getCacheDir(widgetName) {
        if (!widgetName || widgetName === "serpantinum" || cacheDir.endsWith("/" + widgetName)) {
            root.ensureDir(cacheDir);
            return cacheDir;
        }
        var envPath = Quickshell.env("QS_CACHE_" + widgetName.toUpperCase());
        var finalPath = envPath ? envPath : (cacheDir + "/" + widgetName);
        root.ensureDir(finalPath);
        return finalPath;
    }

    function getStateDir(widgetName) {
        if (!widgetName || widgetName === "serpantinum" || stateDir.endsWith("/" + widgetName)) {
            root.ensureDir(stateDir);
            return stateDir;
        }
        var envPath = Quickshell.env("QS_STATE_" + widgetName.toUpperCase());
        var finalPath = envPath ? envPath : (stateDir + "/" + widgetName);
        root.ensureDir(finalPath);
        return finalPath;
    }

    function getRunDir(widgetName) {
        if (!widgetName || widgetName === "serpantinum" || runDir.endsWith("/" + widgetName)) {
            root.ensureDir(runDir);
            return runDir;
        }
        var envPath = Quickshell.env("QS_RUN_" + widgetName.toUpperCase());
        var finalPath = envPath ? envPath : (runDir + "/" + widgetName);
        root.ensureDir(finalPath);
        return finalPath;
    }

    function getLogDir(widgetName) {
        if (!widgetName || widgetName === "serpantinum" || logDir.endsWith("/" + widgetName)) {
            root.ensureDir(logDir);
            return logDir;
        }
        var envPath = Quickshell.env("QS_LOG_" + widgetName.toUpperCase());
        var finalPath = envPath ? envPath : (logDir + "/" + widgetName);
        root.ensureDir(finalPath);
        return finalPath;
    }
}
