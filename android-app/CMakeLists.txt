cmake_minimum_required(VERSION 3.22)
project(uvc_jni_bridge)

add_library(uvc_jni SHARED IMPORTED GLOBAL)
set_target_properties(uvc_jni PROPERTIES IMPORTED_LOCATION ${CMAKE_SOURCE_DIR}/app/src/main/cpp/arm64-v8a/libuvc_jni.so)

add_library(test_app SHARED app/src/main/cpp/test_app.cpp)
find_library(log-lib log)
target_link_libraries(test_app uvc_jni ${log-lib})
