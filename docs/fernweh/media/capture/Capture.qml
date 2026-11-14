import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Controls.Material 2.15
import "qrc:/gui" as Fernweh

ApplicationWindow {
    id: window
    visible: true
    width: 920
    height: capturePage === 0 ? 680 : 900
    color: "#2D3035"
    Material.theme: Material.Dark
    Material.accent: "#5A9CE6"
    Material.primary: "#393D43"
    font.family: "Helvetica Neue"
    font.pointSize: 11

    Item { id: stackView; anchors.fill: parent }
    Fernweh.DisplaySetupDialog {
        id: setup
        Component.onCompleted: openFor("sample-workstation", "Studio workstation", "", false, capturePage)
    }
}
