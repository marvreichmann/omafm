import QtQuick
import Quickshell.Io

Item {
    id: root
    property string server: "127.0.0.1:5259"
    property real frequency: 102.4
    property real volume: 0.3
    property bool muted: false
    // What the backend is actually told to play at. Mute keeps `volume` intact
    // so unmuting returns to the level the slider still shows.
    readonly property real outputVolume: muted ? 0 : volume
    property string status: "Disconnected"
    property string phase: "stopped"
    property bool failed: false
    property bool stereo: false
    property bool stopping: false
    readonly property bool running: backend.running

    function tune(text) {
        const n = Number(String(text).replace(",", "."))
        if (!isFinite(n) || n < 65 || n > 108) {
            status = "Enter an FM frequency between 65 and 108 MHz"
            failed = true
            return false
        }
        frequency = Math.round(n * 10000) / 10000
        failed = false
        if (backend.running) backend.write(JSON.stringify({ frequency: frequency }) + "\n")
        return true
    }
    function setVolume(value) {
        volume = Math.max(0, Math.min(1, value))
        // Moving the slider is an unambiguous request to hear something.
        muted = false
        sendVolume()
    }
    function toggleMute() {
        muted = !muted
        sendVolume()
    }
    function sendVolume() {
        if (backend.running) backend.write(JSON.stringify({ volume: outputVolume }) + "\n")
    }
    function connectServer() {
        if (backend.running) return
        if (!server.trim() || !tune(frequency)) {
            status = "Enter a server address and valid FM frequency"
            failed = true
            return
        }
        failed = false
        stopping = false
        phase = "connecting"
        status = "Connecting…"
        backend.command = [decodeURIComponent(String(Qt.resolvedUrl("bin/omasdr")).replace(/^file:\/\//, "")),
            "--server", server.trim(), "--frequency", String(frequency), "--volume", String(outputVolume)]
        backend.running = true
        launchCheck.restart()
    }
    function disconnectServer() {
        if (!backend.running) return
        stopping = true
        status = "Disconnecting…"
        backend.write('{"stop":true}\n')
        stopTimeout.restart()
    }
    Component.onDestruction: {
        // Closing stdin also disconnects if shell teardown beats this write.
        if (backend.running) backend.write('{"stop":true}\n')
    }
    Timer {
        id: launchCheck
        interval: 1000
        onTriggered: {
            if (!backend.running && root.status === "Connecting…") {
                root.status = "Cannot start bundled backend; check bin/omasdr and its executable permission"
                root.failed = true
            }
        }
    }
    Timer {
        id: stopTimeout
        interval: 2500
        onTriggered: if (backend.running) backend.signal(9)
    }
    Process {
        id: backend
        stdinEnabled: true
        stdout: SplitParser {
            onRead: function(line) {
                try {
                    const message = JSON.parse(line)
                    if (message.state === "stats") return
                    root.phase = String(message.state)
                    if (message.state !== "warning") root.stereo = message.stereo === true
                    root.status = String(message.message || "")
                    root.failed = message.state === "error" || message.state === "warning"
                } catch (error) {
                    root.status = "Invalid response from backend"
                    root.failed = true
                }
            }
        }
        stderr: SplitParser {
            onRead: function(line) {
                if (line.trim()) { root.status = line.trim(); root.failed = true }
            }
        }
        onExited: function(exitCode) {
            stopTimeout.stop()
            root.phase = "stopped"
            root.stereo = false
            if (root.stopping || (exitCode === 0 && !root.failed)) {
                root.status = "Disconnected"
                root.failed = false
            } else if (!root.failed) {
                root.status = "Receiver exited unexpectedly (" + exitCode + ")"
                root.failed = true
            }
            root.stopping = false
        }
    }
}
