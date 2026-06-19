package com.example.uvc

class NativeEngine : AutoCloseable {
    private var handle: Long = 0L

    init {
        handle = initialize()
    }

    external private fun initialize(): Long

    external fun startCamera(
        cameraId: String,
        width: Int,
        height: Int,
        fps: Int,
        frameCount: Long = 0L,
    ): Long

    external fun stopCamera(cameraHandle: Long): Int

    external fun setControl(cameraHandle: Long, name: String, value: Int): Int

    external fun isCameraRunning(cameraHandle: Long): Boolean

    external fun pollFrame(timeoutMs: Int): ByteArray?

    external fun getSupportedFormats(): Array<String>

    external fun getLastError(): String

    external fun getLastErrorCode(): Int

    external fun getCameraCount(): Int

    external private fun releaseEngine(): Int

    override fun close() {
        if (handle != 0L) {
            releaseEngine()
            handle = 0L
        }
    }

    @Deprecated("Use close()", ReplaceWith("close()"))
    protected fun finalize() {
        close()
    }

    companion object {
        const val ERROR_OK: Int = 0
        const val ERROR_NULL_HANDLE: Int = -1
        const val ERROR_INVALID_ARGUMENT: Int = -2
        const val ERROR_ALREADY_RUNNING: Int = -3
        const val ERROR_NOT_RUNNING: Int = -4
        const val ERROR_SINK_CLOSED: Int = -5
        const val ERROR_BACKEND: Int = -6
        const val ERROR_TIMEOUT: Int = -7

        init {
            System.loadLibrary("uvc_jni")
        }
    }
}
