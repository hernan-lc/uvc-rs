package com.test.uvc
import android.app.Activity
import android.os.Bundle
import android.util.Log

class MainActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Log.i("uvc_test", "MainActivity started")
        System.loadLibrary("uvc_jni")
        val rc = nativeInit()
        Log.i("uvc_test", "nativeInit returned $rc")
        finish()
    }
    external fun nativeInit(): Int
}
