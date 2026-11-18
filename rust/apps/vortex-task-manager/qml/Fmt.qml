pragma Singleton
// Cómo se escriben los números: «2,1 GB», «0,4 %», «12,3 Mbps», «3 h 12 min». Coma decimal, como en
// el resto de Vortexine's.
import QtQuick

QtObject {
    function dec(n, d) { return Number(n).toFixed(d).replace(".", ",") }

    function bytes(b) {
        b = Number(b) || 0
        if (b >= 1024 * 1024 * 1024 * 1024) return dec(b / 1099511627776, 1) + " TB"
        if (b >= 1024 * 1024 * 1024) return dec(b / 1073741824, 1) + " GB"
        if (b >= 1024 * 1024) return dec(b / 1048576, b >= 100 * 1048576 ? 0 : 1) + " MB"
        if (b >= 1024) return dec(b / 1024, 0) + " KB"
        return Math.round(b) + " B"
    }

    // Disco: siempre en MB/s (como Windows), con un decimal.
    function mbs(b) { return dec((Number(b) || 0) / 1048576, 1) + " MB/s" }

    // Red: en bits por segundo.
    function bits(b) {
        var v = (Number(b) || 0) * 8
        if (v >= 1e9) return dec(v / 1e9, 1) + " Gbps"
        if (v >= 1e6) return dec(v / 1e6, 1) + " Mbps"
        if (v >= 1e3) return dec(v / 1e3, 0) + " Kbps"
        return Math.round(v) + " bps"
    }

    function pct(v) {
        v = Number(v) || 0
        return (v < 10 ? dec(v, 1) : Math.round(v)) + " %"
    }

    function tiempo(s) {
        s = Math.floor(Number(s) || 0)
        var d = Math.floor(s / 86400), h = Math.floor(s % 86400 / 3600), m = Math.floor(s % 3600 / 60)
        if (d > 0) return d + " d " + h + " h"
        if (h > 0) return h + " h " + m + " min"
        return m + " min"
    }
}
