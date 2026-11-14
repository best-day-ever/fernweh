QT += core gui qml quick quickcontrols2
CONFIG += console c++17
CONFIG -= app_bundle
TEMPLATE = app
TARGET = fernweh-doc-capture
SOURCES += capture.cpp
RESOURCES += "$$(FERNWEH_CAPTURE_RESOURCES)"
macx {
    QMAKE_CXXFLAGS += -include arm_acle.h
    QMAKE_MACOSX_DEPLOYMENT_TARGET = 15.0
}
