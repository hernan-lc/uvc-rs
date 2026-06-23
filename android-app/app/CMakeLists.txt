cmake_minimum_required(VERSION 3.22)
project(uvc_jni_bridge)

add_library(uvc_jni SHARED IMPORTED GLOBAL)
set_target_properties(uvc_jni PROPERTIES IMPORTED_LOCATION ${CMAKE_SOURCE_DIR}/src/main/cpp/${ANDROID_ABI}/libuvc_jni.so)

add_library(test_app SHARED src/main/cpp/test_app.cpp)
find_library(log-lib log)
target_link_libraries(test_app uvc_jni ${log-lib})
