pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.Commons
import qs.Ui as Ui

Ui.Panel {
    id: root
    moduleName: "marv.omasdr"
    manageIpc: false

    property var anchorItem: null
    property var hostWidget: null
    property Receiver receiver: null
    readonly property var hostBar: root.bar
    readonly property var fonts: Style.font
    readonly property var spacing: Style.spacing
    readonly property color foreground: hostBar ? hostBar.foreground : Color.foreground
    readonly property color dim: Qt.darker(foreground, 1.55)
    readonly property string fontFamily: hostBar ? hostBar.fontFamily : fonts.family
    readonly property string phaseLabel: !receiver ? "Ready"
        : receiver.failed ? "Needs attention"
        : receiver.stopping ? "Disconnecting"
        : receiver.phase === "playing" ? "Listening · FM mono"
        : receiver.phase === "tuning" ? "Tuning"
        : receiver.phase === "receiving" ? "Receiving"
        : receiver.running ? "Connecting"
        : "Ready · FM radio"

    Ui.KeyboardPanel {
        id: panel
        anchorItem: root.anchorItem
        owner: root.hostWidget || root
        bar: root.bar
        open: root.opened
        focusTarget: root.receiver && root.receiver.running ? frequencyField : serverField
        contentWidth: panel.fittedContentWidth(Style.space(360))
        contentHeight: panel.fittedContentHeight(content.implicitHeight)

        // Editors keep native selection and Tab navigation, so this panel has
        // no PanelKeyCatcher and gives up Tab-to-adjacent-panel switching: a
        // form needs Tab to reach its own fields. Escape bubbles here, including
        // while a field owns the keyboard focus.
        Item {
            anchors.fill: parent
            Keys.onEscapePressed: root.close()
            ColumnLayout {
                id: content
                width: parent.width
                spacing: root.spacing.md

                Ui.PanelHero {
                    Layout.fillWidth: true
                    title: "OmaSDR"
                    meta: root.phaseLabel
                    foreground: root.foreground
                    fontFamily: root.fontFamily
                    iconComponent: Component {
                        Ui.OpticalGlyph {
                            text: "󰐹"
                            width: root.fonts.display
                            height: root.fonts.display
                            fontSize: root.fonts.display
                            fontFamily: root.fontFamily
                            color: root.foreground
                        }
                    }
                }
                LabeledField {
                    id: serverField
                    Layout.fillWidth: true
                    heading: "SDR++ server"
                    text: root.receiver ? root.receiver.server : ""
                    placeholderText: "host:5259"
                    readOnly: root.receiver && root.receiver.running
                    onTextEdited: if (root.receiver) root.receiver.server = text
                    activeFocusOnTab: !readOnly
                }
                LabeledField {
                    id: frequencyField
                    Layout.fillWidth: true
                    heading: "Frequency · MHz"
                    text: root.receiver ? String(root.receiver.frequency) : "102.4"
                    font.pixelSize: root.fonts.heading
                    onAccepted: if (root.receiver) root.receiver.tune(text)
                    onEditingFinished: if (root.receiver) root.receiver.tune(text)
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: root.spacing.sm
                    RadioButton {
                        Layout.fillWidth: true
                        Layout.preferredWidth: 1
                        text: "− 100 kHz"
                        Accessible.name: "Tune down 100 kilohertz"
                        enabled: root.receiver && root.receiver.frequency > 65
                        onClicked: root.receiver.tune((root.receiver.frequency - 0.1).toFixed(4))
                    }
                    RadioButton {
                        Layout.fillWidth: true
                        Layout.preferredWidth: 1
                        text: "+ 100 kHz"
                        Accessible.name: "Tune up 100 kilohertz"
                        enabled: root.receiver && root.receiver.frequency < 108
                        onClicked: root.receiver.tune((root.receiver.frequency + 0.1).toFixed(4))
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: root.spacing.md
                    Text {
                        text: "Volume"
                        color: root.foreground
                        font.family: root.fontFamily
                        font.pixelSize: root.fonts.bodySmall
                    }
                    Ui.PanelSlider {
                        Layout.fillWidth: true
                        bar: root.bar
                        minimum: 0; maximum: 1; step: 0.01
                        value: root.receiver ? root.receiver.volume : 0.3
                        trackColor: Util.alpha(root.foreground, 0.15)
                        fillColor: root.foreground
                        knobColor: activeFocus ? Color.accent : root.foreground
                        Accessible.name: "Radio volume"
                        activeFocusOnTab: true
                        onMoved: function(value) { if (root.receiver) root.receiver.setVolume(value) }
                        Keys.onLeftPressed: if (root.receiver) root.receiver.setVolume(root.receiver.volume - 0.05)
                        Keys.onRightPressed: if (root.receiver) root.receiver.setVolume(root.receiver.volume + 0.05)
                        Keys.onPressed: function(event) {
                            if (!root.receiver) return
                            if (event.key === Qt.Key_Home || event.key === Qt.Key_End) {
                                root.receiver.setVolume(event.key === Qt.Key_Home ? 0 : 1)
                                event.accepted = true
                            }
                        }
                    }
                    Text {
                        Layout.minimumWidth: Style.space(35)
                        horizontalAlignment: Text.AlignRight
                        text: root.receiver ? Math.round(root.receiver.volume * 100) + "%" : "30%"
                        color: root.dim
                        font.family: root.fontFamily
                        font.pixelSize: root.fonts.bodySmall
                    }
                }
                Ui.PanelSeparator { Layout.fillWidth: true; foreground: root.foreground }
                RadioButton {
                    Layout.fillWidth: true
                    text: root.receiver && root.receiver.running ? "Disconnect" : "Connect"
                    enabled: root.receiver && !root.receiver.stopping
                    onClicked: {
                        if (root.receiver.running) root.receiver.disconnectServer()
                        else if (root.receiver.tune(frequencyField.text)) root.receiver.connectServer()
                    }
                }
                Text {
                    Layout.fillWidth: true
                    text: root.receiver && root.receiver.failed ? root.receiver.status : "Enter to tune · Esc to close"
                    textFormat: Text.PlainText
                    color: root.receiver && root.receiver.failed ? Color.urgent : root.dim
                    font.family: root.fontFamily
                    font.pixelSize: root.fonts.caption
                    wrapMode: Text.WordWrap
                }
            }
        }
    }

    // Thin, flat rows and accent focus like Omathought, using Omarchy tokens.
    component LabeledField: Ui.TextField {
        id: field
        required property string heading
        readonly property bool hot: !readOnly && (activeFocus || hovered)
        foreground: root.foreground
        font.family: root.fontFamily
        font.pixelSize: root.fonts.bodySmall
        selectByMouse: true
        horizontalPadding: root.spacing.md
        topPadding: caption.implicitHeight + root.spacing.md + root.spacing.xs
        bottomPadding: root.spacing.md
        Accessible.name: heading
        background: Ui.BorderSurface {
            radius: Style.cornerRadius
            color: field.hot ? Style.selectedFillFor(root.foreground, Color.accent)
                : Style.normalFillFor(root.foreground, Color.accent)
            borderSpec: Border.controlSpec(field.hot ? "hover-cursor" : "normal",
                field.hot ? Color.accent : root.foreground, Color.accent)
            Behavior on color { ColorAnimation { duration: 120; easing.type: Easing.OutCubic } }
        }
        Text {
            id: caption
            x: field.leftPadding
            y: root.spacing.md
            text: field.heading
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: root.fonts.caption
        }
    }
    component RadioButton: Ui.Button {
        focusable: true
        bordered: true
        foreground: root.foreground
        fontFamily: root.fontFamily
        fontSize: root.fonts.bodySmall
    }
}
