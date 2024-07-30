/*
* Copyright 2024 Alex Hayton / The Jaenokhwa Contributors
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
    use std::{ffi::c_void, sync::Arc, time::Instant};

    #[cfg(target_os = "ios")]
    use av_foundation::capture_device::{
        AVCaptureDeviceTypeBuiltInDualCamera, AVCaptureDeviceTypeBuiltInTelephotoCamera,
        AVCaptureDeviceTypeBuiltInTrueDepthCamera, AVCaptureDeviceTypeBuiltInUltraWideCamera,
    };
    use av_foundation::{
        capture_device::{
            AVCaptureDevice, AVCaptureDeviceFormat, AVCaptureDevicePositionUnspecified,
            AVCaptureDeviceType, AVCaptureDeviceTypeBuiltInWideAngleCamera,
            AVCaptureDeviceTypeContinuityCamera, AVCaptureDeviceTypeDeskViewCamera,
            AVCaptureDeviceTypeExternalUnknown, AVCaptureFocusModeAutoFocus,
            AVCaptureFocusModeContinuousAutoFocus, AVCaptureFocusModeLocked,
        },
        capture_device_discovery_session::AVCaptureDeviceDiscoverySession,
        capture_output_base::AVCaptureOutput,
        capture_session::AVCaptureConnection,
        capture_video_data_output::AVCaptureVideoDataOutputSampleBufferDelegate,
        media_format::AVMediaTypeVideo,
    };
    use core_foundation::base::TCFType;
    use core_media::{
        sample_buffer::{CMSampleBuffer, CMSampleBufferRef},
        time::CMTime,
        OSType,
    };
    use core_video::pixel_buffer::CVPixelBuffer;
    use flume::Sender;
    use four_cc::FourCC;
    use jaenokhwa_core::{
        buffer::FrameBuffer,
        error::NokhwaError,
        types::{
            ApiBackend, CameraControl, CameraFormat, CameraIndex, CameraInfo,
            ControlValueDescription, ControlValueSetter, KnownCameraControl, Resolution,
        },
    };
    use objc2::{
        declare_class, extern_methods, msg_send, msg_send_id, mutability,
        rc::{Allocated, Id, Retained},
        ClassType, DeclaredClass,
    };
    use objc2_foundation::{NSArray, NSObject, NSObjectProtocol, NSString};

    #[allow(non_upper_case_globals)]
    fn raw_fcc_to_fourcc(raw: OSType) -> FourCC {
        FourCC::from(raw)
    }

    pub type SenderType = Sender<FrameBuffer>;

    pub struct DelegateIvars {
        sender: *const c_void,
    }

    declare_class!(
        pub struct AVCaptureDelegate;

        unsafe impl ClassType for AVCaptureDelegate {
            type Super = NSObject;
            type Mutability = mutability::Mutable;
            const NAME: &'static str = "OutputSampleBufferDelegate";
        }

        impl DeclaredClass for AVCaptureDelegate {
            type Ivars = DelegateIvars;
        }

        unsafe impl NSObjectProtocol for AVCaptureDelegate {}

        unsafe impl AVCaptureVideoDataOutputSampleBufferDelegate for AVCaptureDelegate {
            #[method(captureOutput:didOutputSampleBuffer:fromConnection:)]
            unsafe fn capture_output_did_output_sample_buffer(
                &self,
                _capture_output: &AVCaptureOutput,
                sample_buffer: CMSampleBufferRef,
                _connection: &AVCaptureConnection,
            ) {
                let sample_buffer = CMSampleBuffer::wrap_under_get_rule(sample_buffer);
                if let Some(image_buffer) = sample_buffer.get_image_buffer() {
                    if let Some(pixel_buffer) = image_buffer.downcast::<CVPixelBuffer>() {
                        pixel_buffer.lock_base_address(0);
                        let width = pixel_buffer.get_width();
                        let height = pixel_buffer.get_height();
                        let base_address = pixel_buffer.get_base_address();
                        let pixel_format = pixel_buffer.get_pixel_format();
                        let buffer_length = pixel_buffer.get_data_size();

                        // Capture the bytes from the buffer
                        let buffer_as_vec = unsafe {
                            std::slice::from_raw_parts_mut(base_address as *mut u8, buffer_length as usize)
                                .to_vec()
                        };

                        pixel_buffer.unlock_base_address(0);

                        let sender_raw = self.ivars().sender;
                        let sender: Arc<SenderType> = unsafe {
                                    let ptr = sender_raw.cast::<SenderType>();
                                    Arc::from_raw(ptr)
                                };
                        let framebuffer = FrameBuffer::new(Resolution::new(width as u32, height as u32), &buffer_as_vec, raw_fcc_to_fourcc(pixel_format), Instant::now());
                        if let Err(_) = sender.send(framebuffer) {
                            return;
                        }
                        std::mem::forget(sender);
                    }
                }
            }

            #[method(captureOutput:didDropSampleBuffer:fromConnection:)]
            unsafe fn capture_output_did_drop_sample_buffer(
            &self,
            _capture_output: &AVCaptureOutput,
            _sample_buffer: CMSampleBufferRef,
            _connection: &AVCaptureConnection,
            ) {}
        }

        unsafe impl AVCaptureDelegate {
            #[method_id(init)]
            fn init(this: Allocated<Self>) -> Option<Id<Self>> {
                let this = this.set_ivars(DelegateIvars {
                    sender: std::ptr::null(),
                });
                unsafe { msg_send_id![super(this), init] }
            }

            #[method(setSender:)]
            fn __set_sender(&mut self, sender: *const c_void) -> bool {
                self.ivars_mut().sender = sender;
                true
            }
        }
    );

    extern_methods!(
        unsafe impl AVCaptureDelegate {
            #[method_id(new)]
            pub fn new() -> Id<Self>;
        }
    );

    impl AVCaptureDelegate {
        pub fn set_sender(&mut self, sender: Arc<SenderType>) -> bool {
            let raw_sender = Arc::into_raw(sender) as *const c_void;
            return unsafe { msg_send![self, setSender: raw_sender] };
        }
    }

    pub fn query_avfoundation() -> Result<Vec<CameraInfo>, NokhwaError> {
        #[cfg(any(target_os = "macos"))]
        let device_types: Vec<&AVCaptureDeviceType> = unsafe {
            vec![
                AVCaptureDeviceTypeBuiltInWideAngleCamera,
                AVCaptureDeviceTypeContinuityCamera,
                AVCaptureDeviceTypeDeskViewCamera,
                AVCaptureDeviceTypeExternalUnknown,
            ]
        };

        #[cfg(any(target_os = "ios"))]
        let device_types: Vec<&AVCaptureDeviceType> = unsafe {
            vec![
                AVCaptureDeviceTypeBuiltInUltraWideCamera,
                AVCaptureDeviceTypeBuiltInWideAngleCamera,
                AVCaptureDeviceTypeBuiltInTelephotoCamera,
                AVCaptureDeviceTypeBuiltInDualCamera,
                AVCaptureDeviceTypeBuiltInTrueDepthCamera,
                AVCaptureDeviceTypeExternalUnknown,
            ]
        };
        let mut device_types_nsarray = NSArray::new();
        device_types.iter().for_each(|device_type| unsafe {
            device_types_nsarray = device_types_nsarray.arrayByAddingObject(*device_type);
        });
        let discovery_session = unsafe {
            AVCaptureDeviceDiscoverySession::discovery_session_with_device_types(
                &device_types_nsarray,
                AVMediaTypeVideo,
                AVCaptureDevicePositionUnspecified,
            )
        };
        let devices = discovery_session.devices();
        let cameras = devices
            .into_iter()
            .map(|device| get_camera_info(device.as_ref()))
            .collect();
        Ok(cameras)
    }

    pub fn get_camera_info(device: &AVCaptureDevice) -> CameraInfo {
        CameraInfo::new(
            device.unique_id().to_string().as_str(),
            device.localized_name().to_string().as_str(),
            device.manufacturer().to_string().as_str(),
            device.model_id().to_string().as_str(),
            device.device_type().to_string().as_str(),
            device.position().to_string().as_str(),
        )
    }

    pub struct AVCaptureDeviceWrapper {
        inner: Retained<AVCaptureDevice>,
        device: CameraInfo,
        locked: bool,
    }

    impl AVCaptureDeviceWrapper {
        pub fn new(index: &CameraIndex) -> Result<Self, NokhwaError> {
            match &index {
                CameraIndex::Index(idx) => {
                    let devices = query_avfoundation()?;

                    match devices.get(*idx as usize) {
                        Some(device) => Ok(AVCaptureDeviceWrapper::from_unique_id(
                            device.unique_id().as_str(),
                        )?),
                        None => Err(NokhwaError::OpenDeviceError(
                            idx.to_string(),
                            "Not Found".to_string(),
                        )),
                    }
                }
                CameraIndex::String(id) => Ok(AVCaptureDeviceWrapper::from_unique_id(id)?),
            }
        }

        pub fn from_unique_id(unique_id: &str) -> Result<Self, NokhwaError> {
            let binding = NSString::from_str(&unique_id.to_string());
            let nsstr_id = binding.as_ref();
            let device_option =
                av_foundation::capture_device::AVCaptureDevice::device_with_unique_id(nsstr_id);

            if device_option.is_none() {
                return Err(NokhwaError::OpenDeviceError(
                    unique_id.to_string(),
                    "Device is null".to_string(),
                ));
            }
            let device = device_option.unwrap();
            let camera_info = get_camera_info(&device);

            Ok(AVCaptureDeviceWrapper {
                inner: device,
                device: camera_info,
                locked: false,
            })
        }

        pub fn raw_device(&self) -> &AVCaptureDevice {
            &self.inner
        }

        pub fn info(&self) -> &CameraInfo {
            &self.device
        }

        pub fn supported_formats(&self) -> Result<Vec<CameraFormat>, NokhwaError> {
            println!("Formats {:?}", self.inner.formats());

            Ok(self
                .inner
                .formats()
                .into_iter()
                .flat_map(|av_fmt| {
                    let dimensions = av_fmt.video_format_description().get_dimensions();
                    av_fmt
                        .video_supported_frame_rate_ranges()
                        .into_iter()
                        .map(move |fps_f64| {
                            let fps = fps_f64.max_frame_rate() as u32;

                            CameraFormat::new(
                                Resolution::new(dimensions.width as u32, dimensions.height as u32),
                                FourCC::from(av_fmt.format_description().get_media_subtype()),
                                fps,
                            )
                        })
                        .into_iter()
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
            let result = self.inner.lock_for_configuration();
            match result {
                Ok(accepted) => {
                    if accepted {
                        return Ok(());
                    } else {
                        return Err(NokhwaError::SetPropertyError {
                            property: "lockForConfiguration".to_string(),
                            value: "Locked".to_string(),
                            error: "Lock Rejected".to_string(),
                        });
                    }
                }
                Err(_) => {
                    return Err(NokhwaError::SetPropertyError {
                        property: "lockForConfiguration".to_string(),
                        value: "Locked".to_string(),
                        error: "Cannot lock for configuration".to_string(),
                    });
                }
            }
        }

        pub fn unlock(&mut self) {
            if self.locked {
                self.locked = false;
                unsafe { msg_send![&self.inner, unlockForConfiguration] }
            }
        }

        pub fn set_all(&mut self, descriptor: CameraFormat) -> Result<(), NokhwaError> {
            self.lock()?;
            let format_list_raw = self.inner.formats();
            let format_list = format_list_raw.to_vec();

            let mut selected_format: Option<&AVCaptureDeviceFormat> = None;
            let mut min_frame_duration: Option<CMTime> = None;
            let mut max_frame_duration: Option<CMTime> = None;

            for format in format_list {
                let dimensions = format.video_format_description().get_dimensions();

                if dimensions.height == descriptor.resolution().height() as i32
                    && dimensions.width == descriptor.resolution().width() as i32
                {
                    selected_format = Some(format);
                    for range in format.video_supported_frame_rate_ranges() {
                        let max_fps: f64 = range.max_frame_rate();

                        if (f64::from(descriptor.frame_rate()) - max_fps).abs() < 0.01 {
                            min_frame_duration = Some(range.min_frame_duration());
                            max_frame_duration = Some(range.max_frame_duration());
                            break;
                        }
                    }
                }
            }

            if min_frame_duration.is_none()
                || max_frame_duration.is_none()
                || selected_format.is_none()
            {
                return Err(NokhwaError::SetPropertyError {
                    property: "CameraFormat".to_string(),
                    value: descriptor.to_string(),
                    error: "Not Found/Rejected/Unsupported".to_string(),
                });
            }

            self.inner
                .set_active_format(selected_format.expect("selected_format not set"));
            self.inner.set_active_video_min_frame_duration(
                min_frame_duration.expect("min_frame_duration not set"),
            );
            self.inner.set_active_video_max_frame_duration(
                max_frame_duration.expect("max_frame_duration not set"),
            );
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
            let mut controls = vec![];

            let focus_current = self.inner.focus_mode();
            let focus_locked = self.inner.is_focus_mode_supported(AVCaptureFocusModeLocked);
            let focus_auto = self
                .inner
                .is_focus_mode_supported(AVCaptureFocusModeAutoFocus);
            let focus_continuous = self
                .inner
                .is_focus_mode_supported(AVCaptureFocusModeContinuousAutoFocus);

            {
                let mut supported_focus_values = vec![];

                if focus_locked {
                    supported_focus_values.push(AVCaptureFocusModeLocked)
                }
                if focus_auto {
                    supported_focus_values.push(AVCaptureFocusModeAutoFocus)
                }
                if focus_continuous {
                    supported_focus_values.push(AVCaptureFocusModeContinuousAutoFocus)
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

            #[cfg(target_os = "ios")]
            {
                let active_format = self.inner.get_active_format();
                let exposure_duration_min = active_format.min_exposure_duration();
                let exposure_duration_min = active_format.max_exposure_duration();

                {
                    let focus_poi_supported: BOOL =
                        unsafe { msg_send![&self.inner, isFocusPointOfInterestSupported] };
                    let focus_poi: CGPoint =
                        unsafe { msg_send![&self.inner, focusPointOfInterest] };

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

                    let focus_manual: BOOL = unsafe {
                        msg_send![&self.inner, isLockingFocusWithCustomLensPositionSupported]
                    };
                    let focus_lenspos: f32 = unsafe { msg_send![&self.inner, lensPosition] };

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
                    let exposure_current: NSInteger =
                        unsafe { msg_send![&self.inner, exposureMode] };
                    let exposure_locked: BOOL = unsafe {
                        msg_send![&self.inner, isExposureModeSupported:NSInteger::from(0)]
                    };
                    let exposure_auto: BOOL = unsafe {
                        msg_send![&self.inner, isExposureModeSupported:NSInteger::from(1)]
                    };
                    let exposure_continuous: BOOL = unsafe {
                        msg_send![&self.inner, isExposureModeSupported:NSInteger::from(2)]
                    };
                    let exposure_custom: BOOL = unsafe {
                        msg_send![&self.inner, isExposureModeSupported:NSInteger::from(3)]
                    };

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
                        unsafe { msg_send![&self.inner, isExposurePointOfInterestSupported] };
                    let exposure_poi: CGPoint =
                        unsafe { msg_send![&self.inner, exposurePointOfInterest] };

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
                        unsafe { msg_send![&self.inner, isFaceDrivenAutoExposureEnabled] };
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

                    let exposure_bias: f32 = unsafe { msg_send![&self.inner, exposureTargetBias] };
                    let exposure_bias_min: f32 =
                        unsafe { msg_send![&self.inner, minExposureTargetBias] };
                    let exposure_bias_max: f32 =
                        unsafe { msg_send![&self.inner, maxExposureTargetBias] };

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

                    let exposure_duration: CMTime =
                        unsafe { msg_send![&self.inner, exposureDuration] };

                    controls.push(CameraControl::new(
                        KnownCameraControl::Gamma,
                        "ExposureDuration".to_string(),
                        ControlValueDescription::IntegerRange {
                            min: exposure_duration_min.value as isize,
                            max: exposure_duration_max.value as isize,
                            value: exposure_duration.value as isize,
                            step: 1,
                            default: unsafe { AVCaptureExposureDurationCurrent.value } as isize,
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

                    let exposure_iso: f32 = unsafe { msg_send![&self.inner, ISO] };
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
                            default: *unsafe { AVCaptureISOCurrent } as f64,
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
                    let white_balance_mode: NSInteger =
                        unsafe { msg_send![&self.inner, whiteBalanceMode] };
                    let white_balance_manual: BOOL = unsafe {
                        msg_send![&self.inner, isWhiteBalanceModeSupported:NSInteger::from(0)]
                    };
                    let white_balance_auto: BOOL = unsafe {
                        msg_send![&self.inner, isWhiteBalanceModeSupported:NSInteger::from(1)]
                    };
                    let white_balance_continuous: BOOL = unsafe {
                        msg_send![&self.inner, isWhiteBalanceModeSupported:NSInteger::from(2)]
                    };

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
                                value: white_balance_mode,
                                possible,
                                default: 0,
                            },
                            vec![],
                            true,
                        ));
                    }

                    let white_balance_gains: AVCaptureWhiteBalanceGains =
                        unsafe { msg_send![&self.inner, deviceWhiteBalanceGains] };
                    let white_balance_default: AVCaptureWhiteBalanceGains =
                        unsafe { msg_send![&self.inner, grayWorldDeviceWhiteBalanceGains] };
                    let white_balance_max: AVCaptureWhiteBalanceGains =
                        unsafe { msg_send![&self.inner, maxWhiteBalanceGain] };
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

                    let has_torch: BOOL = unsafe { msg_send![&self.inner, isTorchAvailable] };
                    let torch_active: BOOL = unsafe { msg_send![&self.inner, isTorchActive] };
                    let torch_off: BOOL =
                        unsafe { msg_send![&self.inner, isTorchModeSupported:NSInteger::from(0)] };
                    let torch_on: BOOL =
                        unsafe { msg_send![&self.inner, isTorchModeSupported:NSInteger::from(1)] };
                    let torch_auto: BOOL =
                        unsafe { msg_send![&self.inner, isTorchModeSupported:NSInteger::from(2)] };

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
                        self.inner.is_(),
                    ));

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
                }
            }

            Ok(controls)
        }

        pub fn set_control(
            &mut self,
            id: KnownCameraControl,
            value: ControlValueSetter,
        ) -> Result<(), NokhwaError> {
            #[cfg(target_os = "ios")]
            {
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

                        let current_duration = self.inner.exposure_duration();
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
                            msg_send![&self.inner, setExposureModeCustomWithDuration:*current_duration ISO:new_iso completionHandler:Nil]
                        };

                        Ok(())
                    }
                    KnownCameraControl::Gamma => {
                        let duration_ctrl =
                            controls.get(&id).ok_or(NokhwaError::SetPropertyError {
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
                            msg_send![&self.inner, setExposureModeCustomWithDuration:new_duration ISO:current_iso completionHandler:Nil]
                        };

                        Ok(())
                    }
                    KnownCameraControl::WhiteBalance => {
                        let wb_enum_value =
                            controls.get(&id).ok_or(NokhwaError::SetPropertyError {
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
                        let setter = NSInteger::from(*value.as_enum().ok_or(
                            NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Expected Enum".to_string(),
                            },
                        )? as isize);

                        if !wb_enum_value.description().verify_setter(&value) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Failed to verify value".to_string(),
                            });
                        }

                        let _: () = unsafe { msg_send![&self.inner, whiteBalanceMode: setter] };

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

                        let _: () = unsafe { msg_send![&self.inner, whiteBalanceMode: setter] };

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

                        let _: () = unsafe { msg_send![&self.inner, whiteBalanceMode: setter] };

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
                            msg_send![&self.inner, rampToVideoZoomFactor: setter withRate: 1.0_f32]
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

                        let _: () = unsafe { msg_send![&self.inner, exposureMode: setter] };

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

                        let _: () = unsafe { msg_send![&self.inner, focusMode: setter] };

                        Ok(())
                    }
                    _ => Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control not supported".to_string(),
                    }),
                }
            }

            return Err(NokhwaError::SetPropertyError {
                property: id.to_string(),
                value: value.to_string(),
                error: "Control not supported".to_string(),
            });
        }

        pub fn active_format(&self) -> Result<CameraFormat, NokhwaError> {
            let capture_device_format = self.inner.get_active_format();
            let video_format_description = capture_device_format.video_format_description();
            let resolution = video_format_description.get_dimensions();
            let fourcc_bytes = video_format_description.get_codec_type();
            let fourcc = FourCC::from(fourcc_bytes);
            let mut a = capture_device_format
                .video_supported_frame_rate_ranges()
                .into_iter()
                .map(move |range| {
                    let fps = range.max_frame_rate() as u32;
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
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use crate::internal::*;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use av_foundation::capture_input::AVCaptureDeviceInput;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use av_foundation::capture_session::AVCaptureSession;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use av_foundation::capture_video_data_output::AVCaptureVideoDataOutput;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use av_foundation::capture_video_data_output::AVCaptureVideoDataOutputSampleBufferDelegate;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use dispatch2::{Queue, QueueAttribute};
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use objc2::{rc::Retained, runtime::ProtocolObject};
