import QtQuick
import Quickshell
import Quickshell.Io

// Named stations and servers, persisted as user state.
//
// This lives under XDG_STATE_HOME rather than in the widget's Omarchy
// settings: `settings` reaches the widget one-way from the bar's shell.json
// entry, so the panel cannot write a bookmark back into it. The file follows
// the shell's own convention for persistent-but-not-cache state, next to
// notifications.json.
Item {
    id: root

    readonly property string home: Quickshell.env("HOME")
    readonly property string stateDir:
        (Quickshell.env("XDG_STATE_HOME") || home + "/.local/state") + "/omarchy"
    readonly property string path: stateDir + "/omasdr.json"

    // [{ name, frequency }] and [{ name, address }], in user-chosen order.
    property var stations: []
    property var servers: []
    // Writes are refused until the first load settles, so a read failure that
    // is really a slow disk cannot blank an existing file.
    property bool loaded: false

    function addStation(name, frequency) {
        const value = Number(frequency)
        if (!isFinite(value) || value < 65 || value > 108) return false
        stations = stations.concat([{
            name: cleanName(name, value.toFixed(1) + " MHz"),
            frequency: Math.round(value * 10000) / 10000
        }])
        schedule()
        return true
    }
    function addServer(name, address) {
        const value = String(address).trim()
        if (!value) return false
        servers = servers.concat([{ name: cleanName(name, value), address: value }])
        schedule()
        return true
    }
    function renameStation(index, name) { rename("stations", index, name) }
    function renameServer(index, name) { rename("servers", index, name) }
    function removeStation(index) { remove("stations", index) }
    function removeServer(index) { remove("servers", index) }

    // ---------------------------------------------------- internals

    function cleanName(name, fallback) {
        // One line, bounded: these names are drawn in a narrow panel and round
        // -trip through JSON, so newlines and runaway lengths are trimmed here
        // rather than everywhere they are displayed.
        const clean = String(name === undefined ? "" : name).replace(/\s+/g, " ").trim()
        return clean ? clean.slice(0, 48) : fallback
    }
    function rename(list, index, name) {
        const items = root[list].slice()
        if (index < 0 || index >= items.length) return
        const fallback = list === "stations"
            ? Number(items[index].frequency).toFixed(1) + " MHz"
            : items[index].address
        items[index] = Object.assign({}, items[index], { name: cleanName(name, fallback) })
        root[list] = items
        schedule()
    }
    function remove(list, index) {
        const items = root[list].slice()
        if (index < 0 || index >= items.length) return
        items.splice(index, 1)
        root[list] = items
        schedule()
    }
    function schedule() { if (loaded) saveTimer.restart() }

    function load(text) {
        let parsed = {}
        try {
            parsed = text ? JSON.parse(text) : {}
        } catch (error) {
            // A corrupt file is kept, not overwritten: the user's names are
            // worth more than a clean start, and a rename can rescue them.
            console.warn("OmaSDR: ignoring unreadable " + path)
            return
        }
        stations = readList(parsed.stations, function(entry) {
            const value = Number(entry.frequency)
            if (!isFinite(value) || value < 65 || value > 108) return null
            return { name: cleanName(entry.name, value.toFixed(1) + " MHz"), frequency: value }
        })
        servers = readList(parsed.servers, function(entry) {
            const address = String(entry.address === undefined ? "" : entry.address).trim()
            if (!address) return null
            return { name: cleanName(entry.name, address), address: address }
        })
        loaded = true
    }
    function readList(value, convert) {
        if (!Array.isArray(value)) return []
        const out = []
        for (const entry of value) {
            if (!entry || typeof entry !== "object") continue
            const item = convert(entry)
            // A single bad entry drops itself rather than the whole list.
            if (item) out.push(item)
            if (out.length >= 64) break
        }
        return out
    }
    function flush() {
        if (!loaded) return
        file.setText(JSON.stringify({
            version: 1,
            stations: stations,
            servers: servers
        }, null, 2) + "\n")
    }

    Component.onCompleted: ensureDir.running = true
    Process {
        id: ensureDir
        command: ["mkdir", "-p", root.stateDir]
        running: false
        // FileView reports a missing file as a load failure, which is the
        // first-run path: load("") marks the state loaded so the first
        // bookmark actually creates the file.
        onExited: Qt.callLater(function() { file.reload() })
    }
    FileView {
        id: file
        path: root.path
        watchChanges: false
        atomicWrites: true
        printErrors: false
        onLoaded: root.load(text())
        onLoadFailed: root.load("")
    }
    Timer {
        id: saveTimer
        interval: 200
        repeat: false
        onTriggered: root.flush()
    }
}
