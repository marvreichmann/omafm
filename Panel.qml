pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.Commons
import qs.Ui as Ui

Ui.Panel {
    id: root
    moduleName: "com.github.marvreichmann.omasdr"
    manageIpc: false

    property var anchorItem: null
    property var hostWidget: null
    property Receiver receiver: null
    property Bookmarks bookmarks: null
    readonly property var hostBar: root.bar
    readonly property var fonts: Style.font
    readonly property var spacing: Style.spacing
    readonly property color foreground: hostBar ? hostBar.foreground : Color.foreground
    readonly property color dim: Qt.darker(foreground, 1.55)
    readonly property string fontFamily: hostBar ? hostBar.fontFamily : fonts.family
    readonly property string phaseLabel: !receiver ? "Ready"
        : receiver.failed ? "Needs attention"
        : receiver.stopping ? "Disconnecting"
        : receiver.phase === "playing" ? (receiver.stereo ? "Listening · FM stereo" : "Listening · FM mono")
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
            // Alt is the modifier so the digits stay available to the fields:
            // a bare 1-9 would be swallowed while typing a frequency.
            Keys.onPressed: function(event) {
                if (!(event.modifiers & Qt.AltModifier)) return
                const index = event.key - Qt.Key_1
                if (index < 0 || index > 8) return
                event.accepted = true
                const saved = root.bookmarks ? root.bookmarks.stations : []
                if (index < saved.length && root.receiver) root.receiver.tune(saved[index].frequency)
            }
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
                BookmarkStrip {
                    Layout.fillWidth: true
                    // The server cannot change mid-session, so the strip goes
                    // read-only alongside the field it fills in.
                    enabled: !(root.receiver && root.receiver.running)
                    items: root.bookmarks ? root.bookmarks.servers : []
                    emptyHint: "No saved servers"
                    saveHint: "Save this server"
                    currentValue: root.receiver ? root.receiver.server : ""
                    valueOf: function(item) { return item.address }
                    labelOf: function(item) { return item.name }
                    detailOf: function(item) { return item.address }
                    suggestedName: root.receiver ? root.receiver.server : ""
                    canSave: root.receiver && root.receiver.server.trim().length > 0
                        && !(root.receiver && root.receiver.running)
                    onActivated: function(item) { if (root.receiver) root.receiver.server = item.address }
                    onSaved: function(name) {
                        if (root.bookmarks) root.bookmarks.addServer(name, root.receiver.server)
                    }
                    onRenamed: function(index, name) { if (root.bookmarks) root.bookmarks.renameServer(index, name) }
                    onRemoved: function(index) { if (root.bookmarks) root.bookmarks.removeServer(index) }
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
                BookmarkStrip {
                    id: stations
                    Layout.fillWidth: true
                    items: root.bookmarks ? root.bookmarks.stations : []
                    emptyHint: "No saved stations"
                    saveHint: "Save this frequency"
                    currentValue: root.receiver ? Number(root.receiver.frequency).toFixed(4) : ""
                    valueOf: function(item) { return Number(item.frequency).toFixed(4) }
                    labelOf: function(item) { return item.name }
                    detailOf: function(item) { return Number(item.frequency).toFixed(1) + " MHz" }
                    suggestedName: root.receiver
                        ? Number(root.receiver.frequency).toFixed(1) + " MHz" : ""
                    canSave: !!root.receiver
                    onActivated: function(item) { if (root.receiver) root.receiver.tune(item.frequency) }
                    onSaved: function(name) {
                        if (root.bookmarks) root.bookmarks.addStation(name, root.receiver.frequency)
                    }
                    onRenamed: function(index, name) { if (root.bookmarks) root.bookmarks.renameStation(index, name) }
                    onRemoved: function(index) { if (root.bookmarks) root.bookmarks.removeStation(index) }
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
                        text: root.receiver && root.receiver.muted
                            ? "Muted"
                            : (root.receiver ? Math.round(root.receiver.volume * 100) + "%" : "30%")
                        color: root.dim
                        font.family: root.fontFamily
                        font.pixelSize: root.fonts.bodySmall
                    }
                    Ui.PanelActionButton {
                        // Mute leaves the slider where it is, so the button
                        // reads as a state to leave rather than a level change.
                        iconText: root.receiver && root.receiver.muted ? "󰝟" : "󰕾"
                        tooltipText: root.receiver && root.receiver.muted ? "Unmute" : "Mute"
                        Accessible.name: tooltipText
                        foreground: root.receiver && root.receiver.muted ? Color.urgent : root.foreground
                        fontFamily: root.fontFamily
                        focusable: true
                        enabled: !!root.receiver
                        onClicked: root.receiver.toggleMute()
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
                    text: root.receiver && root.receiver.failed ? root.receiver.status
                        : "Alt+1…9 for saved stations · right-click one to edit · Esc to close"
                    textFormat: Text.PlainText
                    color: root.receiver && root.receiver.failed ? Color.urgent : root.dim
                    font.family: root.fontFamily
                    font.pixelSize: root.fonts.caption
                    wrapMode: Text.WordWrap
                }
            }
        }
    }

    // A row of named shortcuts over one list of bookmarks, with an inline
    // editor for creating and renaming them. Both strips in this panel are the
    // same control over different lists; only the accessors differ.
    component BookmarkStrip: ColumnLayout {
        id: strip
        required property var items
        required property string emptyHint
        required property string saveHint
        required property string currentValue
        required property var valueOf
        required property var labelOf
        required property var detailOf
        required property string suggestedName
        required property bool canSave
        signal activated(var item)
        signal saved(string name)
        signal renamed(int index, string name)
        signal removed(int index)

        // -1 is the closed editor, -2 is "naming a new bookmark", and any
        // other value is the index of the bookmark being renamed.
        property int editing: -1
        readonly property bool empty: !items || items.length === 0

        function beginSave() {
            nameField.text = strip.suggestedName
            editing = -2
            nameField.selectAll()
            nameField.forceActiveFocus()
        }
        function beginRename(index) {
            nameField.text = strip.labelOf(strip.items[index])
            editing = index
            nameField.selectAll()
            nameField.forceActiveFocus()
        }
        function commit() {
            const index = editing
            if (index === -2) strip.saved(nameField.text)
            else if (index >= 0 && index < items.length) strip.renamed(index, nameField.text)
            close(index === -2 ? items.length : index)
        }
        // Closing the editor takes the focus with it, so hand it back to the
        // bookmark that was being edited rather than leaving it nowhere.
        function close(focusIndex) {
            editing = -1
            const chip = focusIndex >= 0 && focusIndex < chips.count ? chips.itemAt(focusIndex) : null
            if (chip) chip.forceActiveFocus()
            else addButton.forceActiveFocus()
        }

        spacing: root.spacing.sm

        RowLayout {
            Layout.fillWidth: true
            spacing: root.spacing.sm
            Flow {
                Layout.fillWidth: true
                spacing: root.spacing.sm
                visible: !strip.empty
                Repeater {
                    id: chips
                    model: strip.items
                    delegate: Ui.Button {
                        id: chip
                        required property var modelData
                        required property int index
                        text: strip.labelOf(modelData)
                        // The index doubles as the Alt shortcut for the first
                        // nine, which the panel's footer advertises.
                        tooltipText: strip.detailOf(modelData)
                            + (index < 9 ? " · Alt+" + (index + 1) : "")
                            + " · right-click or F2 to rename or remove"
                        Accessible.name: strip.labelOf(modelData) + ", " + strip.detailOf(modelData)
                        selected: strip.valueOf(modelData) === strip.currentValue
                        focusable: true
                        bordered: true
                        foreground: root.foreground
                        fontFamily: root.fontFamily
                        fontSize: root.fonts.caption
                        onClicked: strip.activated(modelData)
                        // Ui.Button owns a full-size MouseArea, so a child
                        // TapHandler never sees a press: its own signals are the
                        // only way in, and it has no double-click to offer.
                        onRightClicked: strip.beginRename(chip.index)
                        // Right-click has no keyboard equivalent, so a focused
                        // chip answers F2 and the menu key as well.
                        Keys.onPressed: function(event) {
                            if (event.key !== Qt.Key_F2 && event.key !== Qt.Key_Menu) return
                            event.accepted = true
                            strip.beginRename(chip.index)
                        }
                    }
                }
            }
            Text {
                Layout.fillWidth: true
                visible: strip.empty
                text: strip.emptyHint
                color: root.dim
                font.family: root.fontFamily
                font.pixelSize: root.fonts.caption
            }
            Ui.PanelActionButton {
                id: addButton
                iconText: "󰐕"
                tooltipText: strip.saveHint
                Accessible.name: strip.saveHint
                foreground: root.foreground
                fontFamily: root.fontFamily
                focusable: true
                enabled: strip.canSave && strip.editing === -1
                onClicked: strip.beginSave()
            }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: root.spacing.sm
            visible: strip.editing !== -1
            LabeledField {
                id: nameField
                Layout.fillWidth: true
                heading: strip.editing === -2 ? "Name this bookmark" : "Rename bookmark"
                onAccepted: strip.commit()
                Keys.onEscapePressed: function(event) {
                    strip.close(strip.editing)
                    event.accepted = true
                }
            }
            RadioButton {
                text: "Save"
                Accessible.name: "Save bookmark name"
                onClicked: strip.commit()
            }
            RadioButton {
                // Removing from the editor rather than straight off the chip
                // means the name being deleted is on screen first.
                visible: strip.editing >= 0
                text: "Remove"
                Accessible.name: "Remove this bookmark"
                onClicked: {
                    const index = strip.editing
                    strip.close(-1)
                    strip.removed(index)
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
