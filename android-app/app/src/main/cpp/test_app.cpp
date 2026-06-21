#include <jni.h>
#include <android/log.h>

#define LOG_TAG "uvc_test"

extern "C" JNIEXPORT jint JNICALL
Java_com_test_uvc_MainActivity_nativeInit(JNIEnv* env, jobject thiz) {
    __android_log_write(ANDROID_LOG_INFO, LOG_TAG, "nativeInit called");
    return 0;
}
