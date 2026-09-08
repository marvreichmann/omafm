import QtQuick
import qs.Commons
import qs.Ui as Ui

Ui.BarWidget {
    id: root
    moduleName: "com.github.marvreichmann.omasdr"
    readonly property var barStyle: Style.bar

    readonly property Ui.Panel loadedPanel: panelLoader.item as Ui.Panel
    readonly property bool opened: loadedPanel ? loadedPanel.opened : false
    readonly property bool popoutSwitchClosing: loadedPanel ? loadedPanel.popoutSwitchClosing : false
    function open() { if (loadedPanel) loadedPanel.open() }
    function close() { if (loadedPanel) loadedPanel.close() }
    function toggle() { if (loadedPanel) loadedPanel.toggle() }
    function closeForPopoutSwitch() { if (loadedPanel) loadedPanel.closeForPopoutSwitch() }
    function injectPanel() {
        if (!panelLoader.item) return
        panelLoader.item.bar = root.bar
        panelLoader.item.anchorItem = button
        panelLoader.item.hostWidget = root
        panelLoader.item.receiver = receiver
    }
    implicitWidth: button.implicitWidth
    implicitHeight: button.implicitHeight
    onBarChanged: injectPanel()

    Receiver {
        id: receiver
        server: String(root.setting("server", "127.0.0.1:5259"))
        frequency: Number(root.setting("frequency", 102.4))
        volume: Number(root.setting("volume", 0.3))
    }
    Loader {
        id: panelLoader
        active: true
        visible: false
        source: Qt.resolvedUrl("Panel.qml")
        onLoaded: { root.injectPanel(); Qt.callLater(root.injectPanel) }
    }
    Ui.WidgetButton {
        id: button
        anchors.fill: parent
        bar: root.bar
        text: ""
        labelVisible: false
        hasVisualContent: true
        tooltipText: "OmaSDR · " + receiver.status
        Ui.OpticalGlyph {
            anchors.centerIn: parent
            width: root.barStyle.iconCanvas
            height: root.barStyle.iconCanvas
            text: "󰐹"
            fontFamily: button.fontFamily
            fontSize: root.barStyle.iconFont
            color: button.foreground
        }
        Rectangle {
            visible: receiver.phase === "playing"
            width: Style.space(4)
            height: width
            radius: width / 2
            color: button.foreground
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.rightMargin: Style.space(3)
            anchors.bottomMargin: Style.space(3)
        }
        onPressed: function(buttonCode) { if (buttonCode === Qt.LeftButton) root.toggle() }
    }
}
