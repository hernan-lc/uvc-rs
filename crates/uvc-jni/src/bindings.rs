use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use jni::{
    JNIEnv,
    objects::{JClass, JObject, JString},
    sys::{jboolean, jint, jlong, jobject, jobjectArray},
};
use uvc_core::{
    CameraConfig, CameraId, CameraPipeline, EngineError, EngineResult, FrameFormat, FrameReceiver,
    FrameSender, PixelFormat, frame_channel,
};
use uvc_driver::FakeCameraPipeline;

const ERROR_OK: jint = 0;
const ERROR_NULL_HANDLE: jint = -1;
const ERROR_INVALID_ARGUMENT: jint = -2;
const ERROR_ALREADY_RUNNING: jint = -3;
const ERROR_NOT_RUNNING: jint = -4;
const ERROR_SINK_CLOSED: jint = -5;
const ERROR_BACKEND: jint = -6;
const ERROR_TIMEOUT: jint = -7;

pub struct NativeEngine {
    sender: FrameSender,
    receiver: FrameReceiver,
    pipelines: HashMap<i64, FakeCameraPipeline>,
    controls: HashMap<i64, HashMap<String, i32>>,
    next_camera_handle: i64,
    last_error_code: jint,
    last_error: String,
}

impl NativeEngine {
    pub fn new() -> Self {
        let (sender, receiver) = frame_channel(32);

        Self {
            sender,
            receiver,
            pipelines: HashMap::new(),
            controls: HashMap::new(),
            next_camera_handle: 1,
            last_error_code: ERROR_OK,
            last_error: String::new(),
        }
    }

    pub fn start_camera(
        &mut self,
        camera_id: &str,
        width: u32,
        height: u32,
        fps: u32,
        frame_count: Option<u64>,
    ) -> EngineResult<i64> {
        let camera_id = CameraId::new(camera_id)?;
        let format = FrameFormat::new(PixelFormat::Mjpeg, width, height, fps)?;
        let mut config = CameraConfig::new(camera_id, format);

        if let Some(frame_count) = frame_count {
            config = config.with_frame_count(frame_count);
        }

        let handle = self.next_camera_handle;
        let mut pipeline = FakeCameraPipeline::new(config, self.sender.clone());
        pipeline.start()?;

        self.next_camera_handle += 1;
        self.pipelines.insert(handle, pipeline);
        self.controls.insert(handle, HashMap::new());
        self.last_error_code = ERROR_OK;
        self.last_error.clear();

        Ok(handle)
    }

    pub fn stop_camera(&mut self, camera_handle: i64) -> EngineResult<()> {
        let Some(mut pipeline) = self.pipelines.remove(&camera_handle) else {
            let error = EngineError::InvalidArgument(format!(
                "camera handle {camera_handle} is not running"
            ));
            self.record_error(&error);
            return Err(error);
        };

        self.controls.remove(&camera_handle);
        pipeline.stop()
    }

    pub fn set_control(&mut self, camera_handle: i64, name: &str, value: i32) -> EngineResult<()> {
        if name.trim().is_empty() {
            let error = EngineError::InvalidArgument("control name must not be empty".to_owned());
            self.record_error(&error);
            return Err(error);
        }

        let Some(controls) = self.controls.get_mut(&camera_handle) else {
            let error = EngineError::InvalidArgument(format!(
                "camera handle {camera_handle} is not running"
            ));
            self.record_error(&error);
            return Err(error);
        };

        controls.insert(name.to_owned(), value);
        self.last_error_code = ERROR_OK;
        self.last_error.clear();
        Ok(())
    }

    pub fn is_camera_running(&self, camera_handle: i64) -> bool {
        self.pipelines
            .get(&camera_handle)
            .is_some_and(FakeCameraPipeline::is_running)
    }

    pub fn camera_count(&self) -> usize {
        self.pipelines.len()
    }

    pub fn poll_frame(&self, timeout: Duration) -> EngineResult<Option<Vec<u8>>> {
        match self.receiver.recv_timeout(timeout) {
            Ok(frame) => Ok(Some(frame.into_buffer().into_bytes())),
            Err(uvc_core::EngineError::Timeout) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn supported_formats() -> Vec<String> {
        vec![
            "mjpeg".to_owned(),
            "yuyv".to_owned(),
            "h264".to_owned(),
            "nv12".to_owned(),
            "rgba".to_owned(),
        ]
    }

    pub fn last_error_code(&self) -> jint {
        self.last_error_code
    }

    pub fn last_error_message(&self) -> &str {
        &self.last_error
    }

    fn record_error(&mut self, error: &EngineError) {
        self.last_error_code = error_code(error);
        self.last_error = error.to_string();
    }
}

impl Drop for NativeEngine {
    fn drop(&mut self) {
        for (_, mut pipeline) in self.pipelines.drain() {
            let _ = pipeline.stop();
        }
    }
}

pub struct NativeEngineHandle {
    inner: Mutex<NativeEngine>,
}

impl NativeEngineHandle {
    fn new() -> Self {
        Self {
            inner: Mutex::new(NativeEngine::new()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, NativeEngine> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_uvc_NativeEngine_initialize(
    _env: JNIEnv,
    _class: JClass,
) -> jlong {
    Box::into_raw(Box::new(NativeEngineHandle::new())) as jlong
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_uvc_NativeEngine_startCamera(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    camera_id: JString,
    width: jint,
    height: jint,
    fps: jint,
    frame_count: jlong,
) -> jlong {
    let Some(handle) = engine_handle(handle) else {
        return 0;
    };
    let mut engine = handle.lock();

    let camera_id = match jstring_to_string(&mut env, camera_id) {
        Ok(value) => value,
        Err(error) => {
            engine.record_error(&error);
            return 0;
        }
    };

    let frame_count = match frame_count {
        value if value < 0 => {
            let error = EngineError::InvalidArgument(format!(
                "frame_count must be non-negative, got {value}"
            ));
            engine.record_error(&error);
            return 0;
        }
        0 => None,
        value => Some(value as u64),
    };

    match engine.start_camera(
        &camera_id,
        width as u32,
        height as u32,
        fps as u32,
        frame_count,
    ) {
        Ok(camera_handle) => camera_handle,
        Err(error) => {
            throw_jni_error(&mut env, &error);
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_uvc_NativeEngine_stopCamera(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    camera_handle: jlong,
) -> jint {
    let Some(handle) = engine_handle(handle) else {
        return ERROR_NULL_HANDLE;
    };
    let mut engine = handle.lock();

    match engine.stop_camera(camera_handle) {
        Ok(()) => ERROR_OK,
        Err(_) => engine.last_error_code(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_uvc_NativeEngine_setControl(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    camera_handle: jlong,
    name: JString,
    value: jint,
) -> jint {
    let Some(handle) = engine_handle(handle) else {
        return ERROR_NULL_HANDLE;
    };
    let mut engine = handle.lock();

    let name = match jstring_to_string(&mut env, name) {
        Ok(value) => value,
        Err(error) => {
            engine.record_error(&error);
            return engine.last_error_code();
        }
    };

    match engine.set_control(camera_handle, &name, value) {
        Ok(()) => ERROR_OK,
        Err(_) => engine.last_error_code(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_uvc_NativeEngine_isCameraRunning(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    camera_handle: jlong,
) -> jboolean {
    let Some(handle) = engine_handle(handle) else {
        return 0;
    };
    let engine = handle.lock();

    engine.is_camera_running(camera_handle).into()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_uvc_NativeEngine_pollFrame(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    timeout_ms: jint,
) -> jobject {
    let Some(handle) = engine_handle(handle) else {
        return JObject::null().as_raw();
    };
    let mut engine = handle.lock();

    let timeout = Duration::from_millis(timeout_ms.max(0) as u64);

    match engine.poll_frame(timeout) {
        Ok(Some(data)) => match env.byte_array_from_slice(&data) {
            Ok(array) => array.as_raw(),
            Err(error) => {
                engine.record_error(&jni_error(error));
                JObject::null().as_raw()
            }
        },
        Ok(None) => JObject::null().as_raw(),
        Err(error) => {
            engine.record_error(&error);
            JObject::null().as_raw()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_uvc_NativeEngine_getSupportedFormats(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jobjectArray {
    let Some(handle) = engine_handle(handle) else {
        return JObject::null().as_raw();
    };
    let mut engine = handle.lock();

    let formats = NativeEngine::supported_formats();
    let array = match env.new_object_array(
        formats.len() as jni::sys::jsize,
        "java/lang/String",
        JObject::null(),
    ) {
        Ok(array) => array,
        Err(error) => {
            engine.record_error(&jni_error(error));
            return JObject::null().as_raw();
        }
    };

    for (index, format) in formats.iter().enumerate() {
        let value = match env.new_string(format) {
            Ok(value) => value,
            Err(error) => {
                engine.record_error(&jni_error(error));
                return JObject::null().as_raw();
            }
        };

        if let Err(error) = env.set_object_array_element(&array, index as jni::sys::jsize, value) {
            engine.record_error(&jni_error(error));
            return JObject::null().as_raw();
        }
    }

    array.as_raw()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_uvc_NativeEngine_getLastError(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jobject {
    let message = engine_handle(handle)
        .map(|handle| {
            let engine = handle.lock();
            engine.last_error_message().to_owned()
        })
        .unwrap_or_else(|| "invalid native engine handle".to_owned());

    match env.new_string(message) {
        Ok(value) => value.as_raw(),
        Err(_) => JObject::null().as_raw(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_uvc_NativeEngine_getLastErrorCode(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    engine_handle(handle)
        .map(|handle| {
            let engine = handle.lock();
            engine.last_error_code()
        })
        .unwrap_or(ERROR_NULL_HANDLE)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_uvc_NativeEngine_getCameraCount(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    engine_handle(handle)
        .map(|handle| {
            let engine = handle.lock();
            engine.camera_count() as jint
        })
        .unwrap_or(-1)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_uvc_NativeEngine_releaseEngine(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    if handle == 0 {
        return ERROR_NULL_HANDLE;
    }

    unsafe {
        drop(Box::from_raw(handle as *mut NativeEngineHandle));
    }

    ERROR_OK
}

fn engine_handle(handle: jlong) -> Option<&'static NativeEngineHandle> {
    if handle == 0 {
        return None;
    }

    Some(unsafe { &*(handle as *const NativeEngineHandle) })
}

fn jstring_to_string(env: &mut JNIEnv, value: JString) -> EngineResult<String> {
    if value.as_raw().is_null() {
        return Err(EngineError::InvalidArgument(
            "JNI string argument was null".to_owned(),
        ));
    }

    env.get_string(&value)
        .map(|value| value.into())
        .map_err(jni_error)
}

fn throw_jni_error(env: &mut JNIEnv, error: &EngineError) {
    let _ = env.throw_new("java/lang/IllegalStateException", error.to_string());
}

fn error_code(error: &EngineError) -> jint {
    match error {
        EngineError::InvalidCameraId(_)
        | EngineError::InvalidFrameFormat(_)
        | EngineError::InvalidFrameSize { .. }
        | EngineError::InvalidArgument(_) => ERROR_INVALID_ARGUMENT,
        EngineError::SinkClosed => ERROR_SINK_CLOSED,
        EngineError::AlreadyRunning(_) => ERROR_ALREADY_RUNNING,
        EngineError::NotRunning(_) => ERROR_NOT_RUNNING,
        EngineError::Timeout => ERROR_TIMEOUT,
        EngineError::Backend(_) => ERROR_BACKEND,
    }
}

fn jni_error(error: jni::errors::Error) -> EngineError {
    EngineError::Backend(format!("jni error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_engine_starts_stops_and_polls_camera() {
        let mut engine = NativeEngine::new();
        let camera = engine
            .start_camera("usb-cam-1", 16, 16, 30, Some(1))
            .unwrap();

        assert!(engine.is_camera_running(camera));
        assert_eq!(engine.camera_count(), 1);

        let frame = engine.poll_frame(Duration::from_secs(1)).unwrap().unwrap();
        assert!(!frame.is_empty());
        assert_eq!(engine.camera_count(), 1);

        engine.stop_camera(camera).unwrap();
        assert!(!engine.is_camera_running(camera));
        assert_eq!(engine.camera_count(), 0);
    }

    #[test]
    fn native_engine_rejects_unknown_camera_handle() {
        let mut engine = NativeEngine::new();

        assert!(engine.stop_camera(42).is_err());
        assert_eq!(engine.last_error_code(), ERROR_INVALID_ARGUMENT);
    }

    #[test]
    fn native_engine_sets_controls() {
        let mut engine = NativeEngine::new();
        let camera = engine.start_camera("usb-cam-1", 16, 16, 30, None).unwrap();

        engine.set_control(camera, "brightness", 64).unwrap();
        assert!(engine.is_camera_running(camera));

        engine.stop_camera(camera).unwrap();
    }

    #[test]
    fn native_engine_lists_supported_formats() {
        assert_eq!(
            NativeEngine::supported_formats(),
            vec![
                "mjpeg".to_owned(),
                "yuyv".to_owned(),
                "h264".to_owned(),
                "nv12".to_owned(),
                "rgba".to_owned(),
            ]
        );
    }
}
