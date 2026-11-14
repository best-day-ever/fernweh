pragma Singleton
import QtQuick 2.15

// Static, deliberately fictional data matching DisplaySetupController's public
// QML model. These images demonstrate the real views, not planner/hardware tests.
QtObject {
    property string label: "Studio display + MacBook + portrait display"
    property bool planOk: true
    property string planError: ""
    property string hostKind: "arrangement"
    property bool backingExpected: false
    property string scope: "global"
    property bool manual: capturePage === 1
    property string presentation: "windows"
    property string scaling: "native"
    property var clientExtent: ({x: 0, y: 0, width: 4152, height: 1280})
    property var desktop: ({width: 6864, height: 2160, packed: false, captureWidth: 6864, captureHeight: 2160})
    property string summary: "2 screens · 6864 × 2160 desktop · one fullscreen window per screen"
    property var warnings: []
    property var monitors: [
        {key: "sample-studio", name: "Studio display", builtIn: false, notch: false,
         primary: true, on: true, included: true,
         clientX: 0, clientY: 0, clientWidth: 1920, clientHeight: 1080,
         exactText: "3840 × 2160", x: 0, y: 0, width: 3840, height: 2160,
         sizeText: "3840 × 2160", backing: "physical", backingText: "Workstation screen",
         preference: "auto", badge: "exact", badgeText: "Exact pixels", sizeIndex: 0,
         sizeOptions: [{value: "exact", label: "Exact pixels · 3840 × 2160"},
                       {value: "looks-like", label: "Text size · 1920 × 1080"},
                       {value: "custom", label: "Custom…"}]},
        {key: "sample-laptop", name: "MacBook display", builtIn: true, notch: true,
         primary: false, on: true, included: true,
         clientX: 1920, clientY: 135, clientWidth: 1512, clientHeight: 945,
         exactText: "3024 × 1890", x: 3840, y: 270, width: 3024, height: 1890,
         sizeText: "3024 × 1890", backing: "virtual", backingText: "Virtual display, exact size",
         preference: "virtual", badge: "exact", badgeText: "Exact pixels", sizeIndex: 0,
         sizeOptions: [{value: "exact", label: "Exact pixels · 3024 × 1890"},
                       {value: "looks-like", label: "Text size · 1512 × 945"},
                       {value: "custom", label: "Custom…"}]},
        {key: "sample-portrait", name: "Portrait display", builtIn: false, notch: false,
         primary: false, on: false, included: false,
         clientX: 3432, clientY: 0, clientWidth: 720, clientHeight: 1280,
         exactText: "1440 × 2560", x: 0, y: 0, width: 1440, height: 2560,
         sizeText: "", backing: "none", backingText: "", preference: "auto",
         badge: "", badgeText: "", sizeIndex: 0,
         sizeOptions: [{value: "exact", label: "Exact pixels · 1440 × 2560"},
                       {value: "custom", label: "Custom…"}]}
    ]
    signal actionRequested(string action, string key)
    function begin(hostId, hostName, reason) {}
}
