/*
* Copyright 2022 l1npengtul <l1npengtul@protonmail.com> / The Nokhwa Contributors
*
* Licensed under the Apache License, Version 2.0 (the "License");
* you may not use this file except in compliance with the License.
* You may obtain a copy of the License at
*
*     http://www.apache.org/licenses/LICENSE-2.0
*
* Unless required by applicable law or agreed to in writing, software
* distributed under the License is distributed on an "AS IS" BASIS,
* WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
* See the License for the specific language governing permissions and
* limitations under the License.
*/

#![allow(clippy::not_unsafe_ptr_arg_deref)]
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod internal {
    use std::{borrow::Cow, ffi::{c_float, c_void, CStr}, sync::Arc};

    use av_foundation::{capture_device::{AVCaptureDevice, AVCaptureDeviceType, AVCaptureDeviceFormat, AVCaptureDevicePosition}, capture_input::AVCaptureDeviceInput, capture_session::AVCaptureSession, media_format::{AVMediaType, AVMediaTypeText, AVMediaTypeTimecode, AVMediaTypeVideo}};
    use block::ConcreteBlock;
    use core_media::{
        format_description::{CMFormatDescriptionGetMediaSubType, CMFormatDescriptionRef, CMVideoFormatDescriptionGetDimensions},
        sample_buffer::{
            CMSampleBufferGetFormatDescription, CMSampleBufferGetImageBuffer, CMSampleBufferRef,
        },
        time::CMTime,
        OSType,
    };
    use core_video::{
        image_buffer::CVImageBufferRef,
        pixel_buffer::{
            CVPixelBufferGetBaseAddress, CVPixelBufferGetDataSize, CVPixelBufferLockBaseAddress,
            CVPixelBufferUnlockBaseAddress,
        },
    };
    use dispatch::{Queue, QueueAttribute};
    use flume::{Sender, Receiver};
    use four_cc::FourCC;
    use nokhwa_core::{
        error::NokhwaError,
        pixel_format::GRAY,
        types::{
            ApiBackend, CameraControl, CameraFormat, CameraIndex, CameraInfo, ControlValueDescription, ControlValueSetter, KnownCameraControl, KnownCameraControlFlag, Resolution
        },
    };
    use objc2::{
        class, declare::ClassDecl, declare_class, ffi::{Nil, BOOL, NO, YES}, msg_send, runtime::{AnyObject, Class, Protocol, Sel}, sel
    };
    use objc2_foundation::{ns_string, CGFloat, CGPoint, NSArray, NSInteger, NSObject, NSString};
    use once_cell::sync::Lazy;

    #[allow(non_upper_case_globals)]
    fn raw_fcc_to_fourcc(raw: OSType) -> FourCC {
        FourCC::from(raw)
    }

    pub type CompressionData<'a> = (Cow<'a, [u8]>, FourCC);
    pub type DataPipe<'a> = (Sender<CompressionData<'a>>, Receiver<CompressionData<'a>>);

    static CALLBACK_CLASS: Lazy<&'static Class> = Lazy::new(|| {
        {
            let mut decl = ClassDecl::new("MyCaptureCallback", class!(NSObject)).unwrap();

            decl.add_ivar::<*const c_void>("_arcmutptr"); 

            extern "C" fn my_callback_get_arcmutptr(this: &AnyObject, _: Sel) -> *const c_void {
                let ivar = CALLBACK_CLASS::get("MyCaptureCallback").unwrap().instance_variable("_arcmutptr").unwrap();
                unsafe { ivar.load::<*const c_void>(this) }.get()
            }
            extern "C" fn my_callback_set_arcmutptr(
                this: &mut AnyObject,
                _: Sel,
                new_arcmutptr: *const c_void,
            ) {
                let ivar = CALLBACK_CLASS::get("MyCaptureCallback").unwrap().instance_variable("_arcmutptr").unwrap();
                unsafe { ivar.load::<*const c_void>(this) }.set(new_arcmutptr);
            }

            // Delegate compliance method
            // SAFETY: This assumes that the buffer byte size is a u8. Any other size will cause unsafety.
            #[allow(non_snake_case)]
            #[allow(non_upper_case_globals)]
            extern "C" fn capture_out_callback(
                this: &mut AnyObject,
                _: Sel,
                _: *mut AnyObject,
                didOutputSampleBuffer: CMSampleBufferRef,
                _: *mut AnyObject,
            ) {
                let format = unsafe { CMSampleBufferGetFormatDescription(didOutputSampleBuffer) };
                let media_subtype = unsafe { CMFormatDescriptionGetMediaSubType(format) };
                let media_subtype_fcc = FourCC::from(media_subtype);
                println!("media_subtype_fcc: {:?}", media_subtype_fcc);

                let image_buffer: CVImageBufferRef =
                    unsafe { CMSampleBufferGetImageBuffer(didOutputSampleBuffer) };
                unsafe {
                    CVPixelBufferLockBaseAddress(image_buffer, 0);
                };

                let buffer_length = unsafe { CVPixelBufferGetDataSize(image_buffer) };
                let buffer_ptr = unsafe { CVPixelBufferGetBaseAddress(image_buffer) };
                let buffer_as_vec = unsafe {
                    std::slice::from_raw_parts_mut(buffer_ptr as *mut u8, buffer_length as usize)
                        .to_vec()
                };

                unsafe { CVPixelBufferUnlockBaseAddress(image_buffer, 0) };
                // oooooh scarey unsafe
                // AAAAAAAAAAAAAAAAAAAAAAAAA
                // https://c.tenor.com/0e_zWtFLOzQAAAAC/needy-streamer-overload-needy-girl-overdose.gif
                let bufferlck_cv: *const c_void = unsafe { msg_send![this, bufferPtr] };
                let buffer_sndr: Arc<Sender<(Vec<u8>, FourCC)>> = unsafe {
                    let ptr = bufferlck_cv.cast::<Sender<(Vec<u8>, FourCC)>>();
                    Arc::from_raw(ptr)
                };
                if let Err(_) = buffer_sndr.send((buffer_as_vec, GRAY)) {
                    return;
                }
                std::mem::forget(buffer_sndr);
            }

            #[allow(non_snake_case)]
            extern "C" fn capture_drop_callback(
                _: &mut AnyObject,
                _: Sel,
                _: *mut AnyObject,
                _: *mut AnyObject,
                _: *mut AnyObject,
            ) {
            }

            unsafe {
                decl.add_method(
                    sel!(bufferPtr),
                    my_callback_get_arcmutptr as extern "C" fn(&AnyObject, Sel) -> *const c_void,
                );
                decl.add_method(
                    sel!(SetBufferPtr:),
                    my_callback_set_arcmutptr as extern "C" fn(&mut AnyObject, Sel, *const c_void),
                );
                decl.add_method(
                    sel!(captureOutput:didOutputSampleBuffer:fromConnection:),
                    capture_out_callback
                        as extern "C" fn(
                            &mut AnyObject,
                            Sel,
                            *mut AnyObject,
                            CMSampleBufferRef,
                            *mut AnyObject,
                        ),
                );
                decl.add_method(
                    sel!(captureOutput:didDropSampleBuffer:fromConnection:),
                    capture_drop_callback
                        as extern "C" fn(
                            &mut AnyObject,
                            Sel,
                            *mut AnyObject,
                            *mut AnyObject,
                            *mut AnyObject,
                        ),
                );

                decl.add_protocol(
                    Protocol::get("AVCaptureVideoDataOutputSampleBufferDelegate").unwrap(),
                );
            }

            decl.register()
        }
    });

    pub fn request_permission_with_callback(callback: impl Fn(bool) + Send + Sync + 'static) {
        let cls = class!(AVCaptureDevice);

        let wrapper = move |bool: BOOL| {
            callback(bool == YES);
        };

        let objc_fn_block: ConcreteBlock<(BOOL,), (), _> = ConcreteBlock::new(wrapper);
        let objc_fn_pass = objc_fn_block.copy();

        unsafe {
            let _: () = msg_send![cls, requestAccessForMediaType:(AVMediaTypeVideo.clone()) completionHandler:objc_fn_pass];
        }
    }

    pub fn current_authorization_status() -> AVAuthorizationStatus {
        let cls = class!(AVCaptureDevice);
        let status: AVAuthorizationStatus = unsafe {
            msg_send![cls, authorizationStatusForMediaType:AVMediaType::Video.into_ns_str()]
        };
        status
    }

    pub fn query_avfoundation() -> Result<Vec<CameraInfo>, NokhwaError> {
        Ok(AVCaptureDeviceDiscoverySession::new(vec![
            AVCaptureDevice::UltraWide,
            AVCaptureDevice::WideAngle,
            AVCaptureDevice::Telephoto,
            AVCaptureDevice::TrueDepth,
            AVCaptureDevice::ExternalUnknown,
        ])?
        .devices())
    }

    pub fn get_raw_device_info(index: CameraIndex, device: AVCaptureDevice) -> CameraInfo {
        let name = device.localized_name().to_string();
        let uuid = device.unique_id().to_string();
        let manufacturer = device.manufacturer().to_string();
        let position: AVCaptureDevicePosition = device.position();
        let device_type = device.device_type().to_string();
        let model_id = device.model_id().to_string();
        let description = format!(
            "{}: {} - {}, {:?}",
            manufacturer, model_id, device_type, position
        );

        CameraInfo::new(name.as_ref(), &description, &uuid.clone(), index)
    }

    #[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
    #[repr(isize)]
    pub enum AVAuthorizationStatus {
        NotDetermined = 0,
        Restricted = 1,
        Denied = 2,
        Authorized = 3,
    }

    pub struct AVCaptureVideoCallback {
        delegate: *mut AnyObject,
        queue: Queue,
    }

    impl AVCaptureVideoCallback {
        pub fn new(
            device_spec: &CStr,
            buffer: &Arc<Sender<(Vec<u8>, FourCC)>>,
        ) -> Result<Self, NokhwaError> {
            let cls = &CALLBACK_CLASS as &Class;
            let delegate: *mut AnyObject = unsafe { msg_send![cls, alloc] };
            let delegate: *mut AnyObject = unsafe { msg_send![delegate, init] };
            let buffer_as_ptr = {
                let arc_raw = Arc::as_ptr(buffer);
                arc_raw.cast::<c_void>()
            };
            unsafe {
                let _: () = msg_send![delegate, SetBufferPtr: buffer_as_ptr];
            }

            let queue = unsafe {
                Queue::create(device_spec.to_str(), QueueAttribute::Serial)
            };

            Ok(AVCaptureVideoCallback { delegate, queue })
        }

        pub fn data_len(&self) -> usize {
            unsafe { msg_send![self.delegate, dataLength] }
        }

        pub fn inner(&self) -> *mut AnyObject {
            self.delegate
        }

        pub fn queue(&self) -> &Queue {
            &self.queue
        }
    }

    impl AVCaptureDeviceDiscoverySession {
        pub fn new(device_types: Vec<AVCaptureDeviceType>) -> Result<Self, NokhwaError> {
            let device_types = NSArray::from(device_types);
            let position = 0 as NSInteger;

            let discovery_session_cls = class!(AVCaptureDeviceDiscoverySession);
            let discovery_session: *mut AnyObject = unsafe {
                msg_send![discovery_session_cls, discoverySessionWithDeviceTypes:device_types mediaType:AVMediaTypeVideo position:position]
            };

            Ok(AVCaptureDeviceDiscoverySession {
                inner: discovery_session,
            })
        }

        pub fn default() -> Result<Self, NokhwaError> {
            AVCaptureDeviceDiscoverySession::new(vec![
                AVCaptureDevice::UltraWide,
                AVCaptureDevice::Telephoto,
                AVCaptureDevice::ExternalUnknown,
                AVCaptureDevice::Dual,
                AVCaptureDevice::DualWide,
                AVCaptureDevice::Triple,
            ])
        }

        pub fn devices(&self) -> Vec<CameraInfo> {
            let raw_devices = unsafe { msg_send![self.inner, devices] }.to_vec();
            let mut devices = Vec::with_capacity(raw_devices.length());
            for (index, device) in raw_devices.iter().enumerate() {
                devices.push(get_raw_device_info(
                    CameraIndex::Index(index as u32),
                    device,
                ));
            }

            devices
        }
    }

    pub struct AVCaptureDeviceWrapper {
        inner: *mut AVCaptureDevice,
        device: CameraInfo,
        locked: bool,
    }

    impl AVCaptureDeviceWrapper {
        pub fn new(index: &CameraIndex) -> Result<Self, NokhwaError> {
            match &index {
                CameraIndex::Index(idx) => {
                    let devices = query_avfoundation()?;

                    match devices.get(*idx as usize) {
                        Some(device) => Ok(AVCaptureDeviceWrapper::from_id(
                            &device.misc(),
                            Some(index.clone()),
                        )?),
                        None => Err(NokhwaError::OpenDeviceError(
                            idx.to_string(),
                            "Not Found".to_string(),
                        )),
                    }
                }
                CameraIndex::String(id) => Ok(AVCaptureDeviceWrapper::from_id(id, None)?),
            }
        }

        pub fn from_id(id: &str, index_hint: Option<CameraIndex>) -> Result<Self, NokhwaError> {
            let nsstr_id = NSString::from_str(&id.to_string());
            let avfoundation_capture_cls = class!(AVCaptureDevice);
            let capture: *mut AVCaptureDevice =
                unsafe { msg_send![avfoundation_capture_cls, deviceWithUniqueID: nsstr_id] };
            if capture.is_null() {
                return Err(NokhwaError::OpenDeviceError(
                    id.to_string(),
                    "Device is null".to_string(),
                ));
            }
            let camera_info = get_raw_device_info(
                index_hint.unwrap_or_else(|| CameraIndex::String(id.to_string())),
                capture,
            );

            Ok(AVCaptureDeviceWrapper {
                inner: capture,
                device: camera_info,
                locked: false,
            })
        }

        pub fn info(&self) -> &CameraInfo {
            &self.device
        }

        pub fn supported_formats_raw(&self) -> Result<Vec<AVCaptureDeviceFormat>, NokhwaError> {
            unsafe {
                return msg_send![self.inner, formats]
                    .to_vec<AVCaptureDeviceFormat>()
            }
        }

        pub fn supported_formats(&self) -> Result<Vec<CameraFormat>, NokhwaError> {
            Ok(self
                .supported_formats_raw()?
                .into_iter()
                .flat_map(|av_fmt| {
                    let resolution = av_fmt.format_description();
                    av_fmt.video_supported_frame_rate_ranges().iter().map(move |fps_f64| {
                        let fps = *fps_f64 as isize;
                        let format = FourCC::from(av_fmt.format_description().get_media_subtype());

                        let resolution =
                            Resolution::new(resolution.width as u32, resolution.height as u32);
                        CameraFormat::new(resolution, av_fmt.fourcc, fps)
                    })
                })
                .filter(|x| x.frame_rate() != 0)
                .collect())
        }

        pub fn lock(&self) -> Result<(), NokhwaError> {
            if self.locked {
                return Ok(());
            }
            if self.inner.is_in_use_by_another_application() {
                return Err(NokhwaError::InitializeError {
                    backend: ApiBackend::AVFoundation,
                    error: "Already in use".to_string(),
                });
            }
            let err_ptr: *mut c_void = std::ptr::null_mut();
            let accepted: BOOL = unsafe { msg_send![self.inner, lockForConfiguration: err_ptr] };
            if !err_ptr.is_null() {
                return Err(NokhwaError::SetPropertyError {
                    property: "lockForConfiguration".to_string(),
                    value: "Locked".to_string(),
                    error: "Cannot lock for configuration".to_string(),
                });
            }
            // Space these out for debug purposes
            if !accepted == YES {
                return Err(NokhwaError::SetPropertyError {
                    property: "lockForConfiguration".to_string(),
                    value: "Locked".to_string(),
                    error: "Lock Rejected".to_string(),
                });
            }
            Ok(())
        }

        pub fn unlock(&mut self) {
            if self.locked {
                self.locked = false;
                unsafe { msg_send![self.inner, unlockForConfiguration] }
            }
        }

        // thank you ffmpeg
        pub fn set_all(&mut self, descriptor: CameraFormat) -> Result<(), NokhwaError> {
            self.lock()?;
            let format_list_raw = (unsafe { msg_send![self.inner, formats] })?;
            let format_list = format_list_raw.to_vec();
            let format_description_sel = sel!(formatDescription);

            let mut selected_format: *mut AnyObject = std::ptr::null_mut();
            let mut selected_range: *mut AnyObject = std::ptr::null_mut();

            for format in format_list {
                let format_desc_ref: CMFormatDescriptionRef =
                    unsafe { msg_send![format.internal, performSelector: format_description_sel] };
                let dimensions = unsafe { CMVideoFormatDescriptionGetDimensions(format_desc_ref) };

                if dimensions.height == descriptor.resolution().height() as i32
                    && dimensions.width == descriptor.resolution().width() as i32
                {
                    selected_format = format.internal;

                    for range in unsafe {
                        msg_send![format.internal, videoSupportedFrameRateRanges].to_vec()
                    } {
                        let max_fps: f64 = unsafe { msg_send![range.inner, maxFrameRate] };

                        if (f64::from(descriptor.frame_rate()) - max_fps).abs() < 0.01 {
                            selected_range = range.inner;
                            break;
                        }
                    }
                }
            }

            if selected_range.is_null() || selected_format.is_null() {
                return Err(NokhwaError::SetPropertyError {
                    property: "CameraFormat".to_string(),
                    value: descriptor.to_string(),
                    error: "Not Found/Rejected/Unsupported".to_string(),
                });
            }

            let activefmtkey = ns_string!("activeFormat");
            let min_frame_duration = ns_string!("minFrameDuration");
            let active_video_min_frame_duration = ns_string!("activeVideoMinFrameDuration");
            let active_video_max_frame_duration = ns_string!("activeVideoMaxFrameDuration");
            let _: () =
                unsafe { msg_send![self.inner, setValue:selected_format forKey:activefmtkey] };
            let min_frame_duration: *mut AnyObject =
                unsafe { msg_send![selected_range, valueForKey: min_frame_duration] };
            let _: () = unsafe {
                msg_send![self.inner, setValue:min_frame_duration forKey:active_video_min_frame_duration]
            };
            let _: () = unsafe {
                msg_send![self.inner, setValue:min_frame_duration forKey:active_video_max_frame_duration]
            };
            self.unlock();
            Ok(())
        }

        // 0 => Focus POI
        // 1 => Focus Manual Setting
        // 2 => Exposure POI
        // 3 => Exposure Face Driven
        // 4 => Exposure Target Bias
        // 5 => Exposure ISO
        // 6 => Exposure Duration
        pub fn get_controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
            let active_format: *mut AnyObject = unsafe { msg_send![self.inner, activeFormat] };

            let mut controls = vec![];
            // get focus modes

            let focus_current: NSInteger = unsafe { msg_send![self.inner, focusMode] };
            let focus_locked: BOOL =
                unsafe { msg_send![self.inner, isFocusModeSupported:NSInteger::from(0)] };
            let focus_auto: BOOL =
                unsafe { msg_send![self.inner, isFocusModeSupported:NSInteger::from(1)] };
            let focus_continuous: BOOL =
                unsafe { msg_send![self.inner, isFocusModeSupported:NSInteger::from(2)] };

            {
                let mut supported_focus_values = vec![];

                if focus_locked == YES {
                    supported_focus_values.push(0)
                }
                if focus_auto == YES {
                    supported_focus_values.push(1)
                }
                if focus_continuous == YES {
                    supported_focus_values.push(2)
                }

                controls.push(CameraControl::new(
                    KnownCameraControl::Focus,
                    "FocusMode".to_string(),
                    ControlValueDescription::Enum {
                        value: focus_current,
                        possible: supported_focus_values,
                        default: focus_current,
                    },
                    vec![],
                    true,
                ));
            }

            let focus_poi_supported: BOOL =
                unsafe { msg_send![self.inner, isFocusPointOfInterestSupported] };
            let focus_poi: CGPoint = unsafe { msg_send![self.inner, focusPointOfInterest] };

            controls.push(CameraControl::new(
                KnownCameraControl::Other(0),
                "FocusPointOfInterest".to_string(),
                ControlValueDescription::Point {
                    value: (focus_poi.x as f64, focus_poi.y as f64),
                    default: (0.5, 0.5),
                },
                if focus_poi_supported == NO {
                    vec![
                        KnownCameraControlFlag::Disabled,
                        KnownCameraControlFlag::ReadOnly,
                    ]
                } else {
                    vec![]
                },
                focus_auto == YES || focus_continuous == YES,
            ));

            let focus_manual: BOOL =
                unsafe { msg_send![self.inner, isLockingFocusWithCustomLensPositionSupported] };
            let focus_lenspos: f32 = unsafe { msg_send![self.inner, lensPosition] };

            controls.push(CameraControl::new(
                KnownCameraControl::Other(1),
                "FocusManualLensPosition".to_string(),
                ControlValueDescription::FloatRange {
                    min: 0.0,
                    max: 1.0,
                    value: focus_lenspos as f64,
                    step: f64::MIN_POSITIVE,
                    default: 1.0,
                },
                if focus_manual == YES {
                    vec![]
                } else {
                    vec![
                        KnownCameraControlFlag::Disabled,
                        KnownCameraControlFlag::ReadOnly,
                    ]
                },
                focus_manual == YES,
            ));

            // get exposures
            let exposure_current: NSInteger = unsafe { msg_send![self.inner, exposureMode] };
            let exposure_locked: BOOL =
                unsafe { msg_send![self.inner, isExposureModeSupported:NSInteger::from(0)] };
            let exposure_auto: BOOL =
                unsafe { msg_send![self.inner, isExposureModeSupported:NSInteger::from(1)] };
            let exposure_continuous: BOOL =
                unsafe { msg_send![self.inner, isExposureModeSupported:NSInteger::from(2)] };
            let exposure_custom: BOOL =
                unsafe { msg_send![self.inner, isExposureModeSupported:NSInteger::from(3)] };

            {
                let mut supported_exposure_values = vec![];

                if exposure_locked == YES {
                    supported_exposure_values.push(0);
                }
                if exposure_auto == YES {
                    supported_exposure_values.push(1);
                }
                if exposure_continuous == YES {
                    supported_exposure_values.push(2);
                }
                if exposure_custom == YES {
                    supported_exposure_values.push(3);
                }

                controls.push(CameraControl::new(
                    KnownCameraControl::Exposure,
                    "ExposureMode".to_string(),
                    ControlValueDescription::Enum {
                        value: exposure_current,
                        possible: supported_exposure_values,
                        default: exposure_current,
                    },
                    vec![],
                    true,
                ));
            }

            let exposure_poi_supported: BOOL =
                unsafe { msg_send![self.inner, isExposurePointOfInterestSupported] };
            let exposure_poi: CGPoint = unsafe { msg_send![self.inner, exposurePointOfInterest] };

            controls.push(CameraControl::new(
                KnownCameraControl::Other(2),
                "ExposurePointOfInterest".to_string(),
                ControlValueDescription::Point {
                    value: (exposure_poi.x as f64, exposure_poi.y as f64),
                    default: (0.5, 0.5),
                },
                if exposure_poi_supported == NO {
                    vec![
                        KnownCameraControlFlag::Disabled,
                        KnownCameraControlFlag::ReadOnly,
                    ]
                } else {
                    vec![]
                },
                focus_auto == YES || focus_continuous == YES,
            ));

            let expposure_face_driven_supported: BOOL =
                unsafe { msg_send![self.inner, isFaceDrivenAutoExposureEnabled] };
            let exposure_face_driven: BOOL = unsafe {
                msg_send![
                    self.inner,
                    automaticallyAdjustsFaceDrivenAutoExposureEnabled
                ]
            };

            controls.push(CameraControl::new(
                KnownCameraControl::Other(3),
                "ExposureFaceDriven".to_string(),
                ControlValueDescription::Boolean {
                    value: exposure_face_driven == YES,
                    default: false,
                },
                if expposure_face_driven_supported == NO {
                    vec![
                        KnownCameraControlFlag::Disabled,
                        KnownCameraControlFlag::ReadOnly,
                    ]
                } else {
                    vec![]
                },
                exposure_poi_supported == YES,
            ));

            let exposure_bias: f32 = unsafe { msg_send![self.inner, exposureTargetBias] };
            let exposure_bias_min: f32 = unsafe { msg_send![self.inner, minExposureTargetBias] };
            let exposure_bias_max: f32 = unsafe { msg_send![self.inner, maxExposureTargetBias] };

            controls.push(CameraControl::new(
                KnownCameraControl::Other(4),
                "ExposureBiasTarget".to_string(),
                ControlValueDescription::FloatRange {
                    min: exposure_bias_min as f64,
                    max: exposure_bias_max as f64,
                    value: exposure_bias as f64,
                    step: f32::MIN_POSITIVE as f64,
                    default: 0 as f64,
                },
                vec![],
                true,
            ));

            let exposure_duration: CMTime = unsafe { msg_send![self.inner, exposureDuration] };
            let exposure_duration_min: CMTime =
                unsafe { msg_send![active_format, minExposureDuration] };
            let exposure_duration_max: CMTime =
                unsafe { msg_send![active_format, maxExposureDuration] };

            controls.push(CameraControl::new(
                KnownCameraControl::Gamma,
                "ExposureDuration".to_string(),
                ControlValueDescription::IntegerRange {
                    min: exposure_duration_min.value as isize,
                    max: exposure_duration_max.value as isize,
                    value: exposure_duration.value as isize,
                    step: 1,
                    default: unsafe { AVCaptureExposureDurationCurrent.value },
                },
                if exposure_custom == YES {
                    vec![
                        KnownCameraControlFlag::ReadOnly,
                        KnownCameraControlFlag::Volatile,
                    ]
                } else {
                    vec![KnownCameraControlFlag::Volatile]
                },
                exposure_custom == YES,
            ));

            let exposure_iso: f32 = unsafe { msg_send![self.inner, ISO] };
            let exposure_iso_min: f32 = unsafe { msg_send![active_format, minISO] };
            let exposure_iso_max: f32 = unsafe { msg_send![active_format, maxISO] };

            controls.push(CameraControl::new(
                KnownCameraControl::Brightness,
                "ExposureISO".to_string(),
                ControlValueDescription::FloatRange {
                    min: exposure_iso_min as f64,
                    max: exposure_iso_max as f64,
                    value: exposure_iso as f64,
                    step: f32::MIN_POSITIVE as f64,
                    default: unsafe { AVCaptureISO } as f64,
                },
                if exposure_custom == YES {
                    vec![
                        KnownCameraControlFlag::ReadOnly,
                        KnownCameraControlFlag::Volatile,
                    ]
                } else {
                    vec![KnownCameraControlFlag::Volatile]
                },
                exposure_custom == YES,
            ));

            // get whiteblaance
            let white_balance_: NSInteger =
                unsafe { msg_send![self.inner, whiteBalanceMode] };
            let white_balance_manual: BOOL =
                unsafe { msg_send![self.inner, isWhiteBalanceModeSupported:NSInteger::from(0)] };
            let white_balance_auto: BOOL =
                unsafe { msg_send![self.inner, isWhiteBalanceModeSupported:NSInteger::from(1)] };
            let white_balance_continuous: BOOL =
                unsafe { msg_send![self.inner, isWhiteBalanceModeSupported:NSInteger::from(2)] };

            {
                let mut possible = vec![];

                if white_balance_manual == YES {
                    possible.push(0);
                }
                if white_balance_auto == YES {
                    possible.push(1);
                }
                if white_balance_continuous == YES {
                    possible.push(2);
                }

                controls.push(CameraControl::new(
                    KnownCameraControl::WhiteBalance,
                    "WhiteBalanceMode".to_string(),
                    ControlValueDescription::Enum {
                        value: white_balance_,
                        possible,
                        default: 0,
                    },
                    vec![],
                    true,
                ));
            }

            let white_balance_gains: AVCaptureWhiteBalanceGains =
                unsafe { msg_send![self.inner, deviceWhiteBalanceGains] };
            let white_balance_default: AVCaptureWhiteBalanceGains =
                unsafe { msg_send![self.inner, grayWorldDeviceWhiteBalanceGains] };
            let white_balance_max: AVCaptureWhiteBalanceGains =
                unsafe { msg_send![self.inner, maxWhiteBalanceGain] };
            let white_balance_gain_supported: BOOL = unsafe {
                msg_send![
                    self.inner,
                    isLockingWhiteBalanceWithCustomDeviceGainsSupported
                ]
            };

            controls.push(CameraControl::new(
                KnownCameraControl::Gain,
                "WhiteBalanceGain".to_string(),
                ControlValueDescription::RGB {
                    value: (
                        white_balance_gains.redGain as f64,
                        white_balance_gains.greenGain as f64,
                        white_balance_gains.blueGain as f64,
                    ),
                    max: (
                        white_balance_max.redGain as f64,
                        white_balance_max.greenGain as f64,
                        white_balance_max.blueGain as f64,
                    ),
                    default: (
                        white_balance_default.redGain as f64,
                        white_balance_default.greenGain as f64,
                        white_balance_default.blueGain as f64,
                    ),
                },
                if white_balance_gain_supported == YES {
                    vec![
                        KnownCameraControlFlag::Disabled,
                        KnownCameraControlFlag::ReadOnly,
                    ]
                } else {
                    vec![]
                },
                white_balance_gain_supported == YES,
            ));

            // get flash
            let has_torch: BOOL = unsafe { msg_send![self.inner, isTorchAvailable] };
            let torch_active: BOOL = unsafe { msg_send![self.inner, isTorchActive] };
            let torch_off: BOOL =
                unsafe { msg_send![self.inner, isTorchModeSupported:NSInteger::from(0)] };
            let torch_on: BOOL =
                unsafe { msg_send![self.inner, isTorchModeSupported:NSInteger::from(1)] };
            let torch_auto: BOOL =
                unsafe { msg_send![self.inner, isTorchModeSupported:NSInteger::from(2)] };

            {
                let mut possible = vec![];

                if torch_off == YES {
                    possible.push(0);
                }
                if torch_on == YES {
                    possible.push(1);
                }
                if torch_auto == YES {
                    possible.push(2);
                }

                controls.push(CameraControl::new(
                    KnownCameraControl::Other(5),
                    "TorchMode".to_string(),
                    ControlValueDescription::Enum {
                        value: (torch_active == YES) as isize,
                        possible,
                        default: 0,
                    },
                    if has_torch == YES {
                        vec![
                            KnownCameraControlFlag::Disabled,
                            KnownCameraControlFlag::ReadOnly,
                        ]
                    } else {
                        vec![]
                    },
                    has_torch == YES,
                ));
            }

            // get low light boost
            let has_llb: BOOL = unsafe { msg_send![self.inner, isLowLightBoostSupported] };
            let llb_enabled: BOOL = unsafe { msg_send![self.inner, isLowLightBoostEnabled] };

            {
                controls.push(CameraControl::new(
                    KnownCameraControl::BacklightComp,
                    "LowLightCompensation".to_string(),
                    ControlValueDescription::Boolean {
                        value: llb_enabled == YES,
                        default: false,
                    },
                    if has_llb == NO {
                        vec![
                            KnownCameraControlFlag::Disabled,
                            KnownCameraControlFlag::ReadOnly,
                        ]
                    } else {
                        vec![]
                    },
                    has_llb == YES,
                ));
            }

            // get zoom factor
            let zoom_: CGFloat = unsafe { msg_send![self.inner, videoZoomFactor] };
            let zoom_min: CGFloat = unsafe { msg_send![self.inner, minAvailableVideoZoomFactor] };
            let zoom_max: CGFloat = unsafe { msg_send![self.inner, maxAvailableVideoZoomFactor] };

            controls.push(CameraControl::new(
                KnownCameraControl::Zoom,
                "Zoom".to_string(),
                ControlValueDescription::FloatRange {
                    min: zoom_min as f64,
                    max: zoom_max as f64,
                    value: zoom_current as f64,
                    step: f32::MIN_POSITIVE as f64,
                    default: 1.0,
                },
                vec![],
                true,
            ));

            // zoom distortion correction
            let distortion_correction_supported: BOOL =
                unsafe { msg_send![self.inner, isGeometricDistortionCorrectionSupported] };
            let distortion_correction_current_value: BOOL =
                unsafe { msg_send![self.inner, isGeometricDistortionCorrectionEnabled] };

            controls.push(CameraControl::new(
                KnownCameraControl::Other(6),
                "DistortionCorrection".to_string(),
                ControlValueDescription::Boolean {
                    value: distortion_correction_current_value == YES,
                    default: false,
                },
                if distortion_correction_supported == YES {
                    vec![
                        KnownCameraControlFlag::ReadOnly,
                        KnownCameraControlFlag::Disabled,
                    ]
                } else {
                    vec![]
                },
                distortion_correction_supported == YES,
            ));

            Ok(controls)
        }

        pub fn set_control(
            &mut self,
            id: KnownCameraControl,
            value: ControlValueSetter,
        ) -> Result<(), NokhwaError> {
            let rc = self.get_controls()?;
            let controls = rc
                .iter()
                .map(|cc| (cc.control(), cc))
                .collect::<BTreeMap<_, _>>();

            match id {
                KnownCameraControl::Brightness => {
                    let isoctrl = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if isoctrl.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error:
                                "Exposure is in improper state to set ISO (Please set to `custom`!)"
                                    .to_string(),
                        });
                    }

                    if isoctrl.flag().contains(&KnownCameraControlFlag::Disabled) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }

                    let current_duration = unsafe { AVCaptureExposureDurationCurrent };
                    let new_iso = *value.as_float().ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Expected float".to_string(),
                    })? as f32;

                    if !isoctrl.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    let _: () = unsafe {
                        msg_send![self.inner, setExposureModeCustomWithDuration:current_duration ISO:new_iso completionHandler:Nil]
                    };

                    Ok(())
                }
                KnownCameraControl::Gamma => {
                    let duration_ctrl = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if duration_ctrl
                        .flag()
                        .contains(&KnownCameraControlFlag::ReadOnly)
                    {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Exposure is in improper state to set Duration (Please set to `custom`!)"
                                .to_string(),
                        });
                    }

                    if duration_ctrl
                        .flag()
                        .contains(&KnownCameraControlFlag::Disabled)
                    {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }
                    let current_duration: CMTime =
                        self.inner.exposure_duration().unwrap_or(CMTime::default());

                    let current_iso = unsafe { AVCaptureISOCurrent };
                    let new_duration = CMTime {
                        value: *value.as_integer().ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected i64".to_string(),
                        })? as i64,
                        timescale: current_duration.timescale,
                        flags: current_duration.flags,
                        epoch: current_duration.epoch,
                    };

                    if !duration_ctrl.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    let _: () = unsafe {
                        msg_send![self.inner, setExposureModeCustomWithDuration:new_duration ISO:current_iso completionHandler:Nil]
                    };

                    Ok(())
                }
                KnownCameraControl::WhiteBalance => {
                    let wb_enum_value = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if wb_enum_value
                        .flag()
                        .contains(&KnownCameraControlFlag::ReadOnly)
                    {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Read Only".to_string(),
                        });
                    }

                    if wb_enum_value
                        .flag()
                        .contains(&KnownCameraControlFlag::Disabled)
                    {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }
                    let setter =
                        NSInteger::from(*value.as_enum().ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected Enum".to_string(),
                        })? as isize);

                    if !wb_enum_value.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    let _: () = unsafe { msg_send![self.inner, whiteBalanceMode: setter] };

                    Ok(())
                }
                KnownCameraControl::BacklightComp => {
                    let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Read Only".to_string(),
                        });
                    }

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }

                    let setter =
                        NSInteger::from(*value.as_enum().ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected Enum".to_string(),
                        })? as isize);

                    if !ctrlvalue.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    let _: () = unsafe { msg_send![self.inner, whiteBalanceMode: setter] };

                    Ok(())
                }
                KnownCameraControl::Gain => {
                    let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Read Only".to_string(),
                        });
                    }

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }

                    let setter = NSInteger::from(*value.as_boolean().ok_or(
                        NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected Boolean".to_string(),
                        },
                    )? as i32);

                    if !ctrlvalue.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    let _: () = unsafe { msg_send![self.inner, whiteBalanceMode: setter] };

                    Ok(())
                }
                KnownCameraControl::Zoom => {
                    let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Read Only".to_string(),
                        });
                    }

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }

                    let setter = *value.as_float().ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Expected float".to_string(),
                    })? as c_float;

                    if !ctrlvalue.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    let _: () = unsafe {
                        msg_send![self.inner, rampToVideoZoomFactor: setter withRate: 1.0_f32]
                    };

                    Ok(())
                }
                KnownCameraControl::Exposure => {
                    let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Read Only".to_string(),
                        });
                    }

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }

                    let setter =
                        NSInteger::from(*value.as_enum().ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected Enum".to_string(),
                        })? as isize);

                    if !ctrlvalue.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    let _: () = unsafe { msg_send![self.inner, exposureMode: setter] };

                    Ok(())
                }
                KnownCameraControl::Iris => Err(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Read Only".to_string(),
                }),
                KnownCameraControl::Focus => {
                    let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Read Only".to_string(),
                        });
                    }

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }

                    let setter =
                        NSInteger::from(*value.as_enum().ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected Enum".to_string(),
                        })? as isize);

                    if !ctrlvalue.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    let _: () = unsafe { msg_send![self.inner, focusMode: setter] };

                    Ok(())
                }
                KnownCameraControl::Other(i) => match i {
                    0 => {
                        let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Control does not exist".to_string(),
                        })?;

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Read Only".to_string(),
                            });
                        }

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Disabled".to_string(),
                            });
                        }

                        let setter = value
                            .as_point()
                            .ok_or(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Expected Point".to_string(),
                            })
                            .map(|(x, y)| CGPoint {
                                x: *x as f64,
                                y: *y as f64,
                            })?;

                        if !ctrlvalue.description().verify_setter(&value) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Failed to verify value".to_string(),
                            });
                        }

                        let _: () = unsafe { msg_send![self.inner, focusPointOfInterest: setter] };

                        Ok(())
                    }
                    1 => {
                        let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Control does not exist".to_string(),
                        })?;

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Read Only".to_string(),
                            });
                        }

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Disabled".to_string(),
                            });
                        }

                        let setter = *value.as_float().ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected float".to_string(),
                        })? as c_float;

                        if !ctrlvalue.description().verify_setter(&value) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Failed to verify value".to_string(),
                            });
                        }

                        let _: () = unsafe {
                            msg_send![self.inner, setFocusModeLockedWithLensPosition: setter handler: Nil]
                        };

                        Ok(())
                    }
                    2 => {
                        let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Control does not exist".to_string(),
                        })?;

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Read Only".to_string(),
                            });
                        }

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Disabled".to_string(),
                            });
                        }

                        let setter = value
                            .as_point()
                            .ok_or(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Expected Point".to_string(),
                            })
                            .map(|(x, y)| CGPoint {
                                x: *x as f64,
                                y: *y as f64,
                            })?;

                        if !ctrlvalue.description().verify_setter(&value) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Failed to verify value".to_string(),
                            });
                        }

                        let _: () =
                            unsafe { msg_send![self.inner, exposurePointOfInterest: setter] };

                        Ok(())
                    }
                    3 => {
                        let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Control does not exist".to_string(),
                        })?;

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Read Only".to_string(),
                            });
                        }

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Disabled".to_string(),
                            });
                        }

                        let setter =
                            if *value.as_boolean().ok_or(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Expected Boolean".to_string(),
                            })? {
                                YES
                            } else {
                                NO
                            };

                        if !ctrlvalue.description().verify_setter(&value) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Failed to verify value".to_string(),
                            });
                        }

                        let _: () = unsafe {
                            msg_send![
                                self.inner,
                                automaticallyAdjustsFaceDrivenAutoExposureEnabled: setter
                            ]
                        };

                        Ok(())
                    }
                    4 => {
                        let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Control does not exist".to_string(),
                        })?;

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Read Only".to_string(),
                            });
                        }

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Disabled".to_string(),
                            });
                        }

                        let setter = *value.as_float().ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected Float".to_string(),
                        })? as f32;

                        if !ctrlvalue.description().verify_setter(&value) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Failed to verify value".to_string(),
                            });
                        }

                        let _: () = unsafe {
                            msg_send![self.inner, setExposureTargetBias: setter handler: Nil]
                        };

                        Ok(())
                    }
                    5 => {
                        let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Control does not exist".to_string(),
                        })?;

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Read Only".to_string(),
                            });
                        }

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Disabled".to_string(),
                            });
                        }

                        let setter = NSInteger::from(*value.as_enum().ok_or(
                            NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Expected Enum".to_string(),
                            },
                        )? as isize);

                        if !ctrlvalue.description().verify_setter(&value) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Failed to verify value".to_string(),
                            });
                        }

                        let _: () = unsafe { msg_send![self.inner, torchMode: setter] };

                        Ok(())
                    }
                    6 => {
                        let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Control does not exist".to_string(),
                        })?;

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Read Only".to_string(),
                            });
                        }

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Disabled".to_string(),
                            });
                        }

                        let setter =
                            if *value.as_boolean().ok_or(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Expected Boolean".to_string(),
                            })? {
                                YES
                            } else {
                                NO
                            };

                        if !ctrlvalue.description().verify_setter(&value) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Failed to verify value".to_string(),
                            });
                        }

                        let _: () = unsafe {
                            msg_send![self.inner, geometricDistortionCorrectionEnabled: setter]
                        };

                        Ok(())
                    }
                    _ => Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Unknown Control".to_string(),
                    }),
                },
                _ => Err(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Unknown Control".to_string(),
                }),
            }
        }

        pub fn active_format(&self) -> Result<CameraFormat, NokhwaError> {
            let avf_format: AVCaptureDeviceFormat = unsafe { msg_send![self.inner, activeFormat] };
            let resolution = avf_format.resolution;
            let fourcc = avf_format.fourcc;
            let mut a = avf_format
                .fps_list
                .into_iter()
                .map(move |fps_f64| {
                    let fps = fps_f64 as u32;

                    let resolution =
                        Resolution::new(resolution.width as u32, resolution.height as u32); // FIXME: what the fuck?
                    CameraFormat::new(resolution, fourcc, fps)
                })
                .collect::<Vec<_>>();
            a.sort_by(|a, b| a.frame_rate().cmp(&b.frame_rate()));

            if a.len() != 0 {
                Ok(a[a.len() - 1])
            } else {
                Err(NokhwaError::GetPropertyError {
                    property: "activeFormat".to_string(),
                    error: "None??".to_string(),
                })
            }
        }
    }

    pub struct AVCaptureVideoDataOutput {
        inner: *mut AnyObject,
    }

    impl AVCaptureVideoDataOutput {
        pub fn new() -> Self {
            AVCaptureVideoDataOutput::default()
        }

        pub fn add_delegate(&self, delegate: &AVCaptureVideoCallback) -> Result<(), NokhwaError> {
            unsafe {
                let _: () = msg_send![
                    self.inner,
                    setSampleBufferDelegate: delegate.delegate
                    queue: delegate.queue()
                ];
            };
            Ok(())
        }
    }

    impl Default for AVCaptureVideoDataOutput {
        fn default() -> Self {
            let cls = class!(AVCaptureVideoDataOutput);
            let inner: *mut AnyObject = unsafe { msg_send![cls, new] };

            AVCaptureVideoDataOutput { inner }
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use crate::internal::*;
