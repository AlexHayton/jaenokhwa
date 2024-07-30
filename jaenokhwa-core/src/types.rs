use crate::{error::NokhwaError, pixel_format::MJPEG};
use four_cc::FourCC;
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    fmt::{Display, Formatter},
};

/// Tells the init function what camera format to pick.
/// - `AbsoluteHighestResolution`: Pick the highest [`Resolution`], then pick the highest frame rate of those provided.
/// - `AbsoluteHighestFrameRate`: Pick the highest frame rate, then the highest [`Resolution`].
/// - `HighestResolution(Option<u32>)`: Pick the highest [`Resolution`] for the given framerate (the `Option<u32>`). If its `None`, it will pick the highest possible [`Resolution`]
/// - `HighestFrameRate(Option<Resolution>)`: Pick the highest frame rate for the given [`Resolution`] (the `Option<Resolution>`). If it is `None`, it will pick the highest possinle framerate.
/// - `Exact`: Pick the exact [`CameraFormat`] provided.
/// - `Closest`: Pick the closest [`CameraFormat`] provided in order of [`FrameFormat`], [`Resolution`], and FPS. Note that if the [`FrameFormat`] does not exist, this will fail to resolve.
/// - `None`: Pick a random [`CameraFormat`]
#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Default)]
pub enum RequestedFormatType {
    AbsoluteHighestResolution,
    AbsoluteHighestFrameRate,
    HighestResolution(Resolution),
    HighestFrameRate(u32),
    Closest(CameraFormat),
    #[default]
    None,
}

impl Display for RequestedFormatType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// A request to the camera for a valid [`CameraFormat`]
#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct RequestedFormat {
    requested_format: RequestedFormatType,
}

impl RequestedFormat {
    /// Creates a new [`RequestedFormat`] by using the [`RequestedFormatType`] and getting the [`FrameFormat`]
    /// constraints from a generic type.
    #[must_use]
    pub fn new(requested: RequestedFormatType) -> RequestedFormat {
        RequestedFormat {
            requested_format: requested,
        }
    }

    /// Gets the [`RequestedFormatType`]
    #[must_use]
    pub fn from_camera_format(format: CameraFormat) -> RequestedFormat {
        RequestedFormat {
            requested_format: RequestedFormatType::Closest(format),
        }
    }

    /// Fulfill the requested using a list of all available formats.
    ///
    /// See [`RequestedFormatType`] for more details.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn fulfill(&self, all_formats: &[CameraFormat]) -> Option<CameraFormat> {
        match self.requested_format {
            RequestedFormatType::AbsoluteHighestResolution => {
                let mut formats = all_formats.to_vec();
                formats.sort_by_key(CameraFormat::resolution);
                let resolution = *formats.iter().last()?;
                let mut format_resolutions = formats
                    .into_iter()
                    .filter(|fmt| fmt.resolution() == resolution.resolution())
                    .collect::<Vec<CameraFormat>>();
                format_resolutions.sort_by_key(CameraFormat::frame_rate);
                format_resolutions.last().copied()
            }
            RequestedFormatType::AbsoluteHighestFrameRate => {
                let mut formats = all_formats.to_vec();
                formats.sort_by_key(CameraFormat::frame_rate);
                let frame_rate = *formats.iter().last()?;
                let mut format_framerates = formats
                    .into_iter()
                    .filter(|fmt| fmt.frame_rate() == frame_rate.frame_rate())
                    .collect::<Vec<CameraFormat>>();
                format_framerates.sort_by_key(CameraFormat::resolution);
                format_framerates.last().copied()
            }
            RequestedFormatType::HighestResolution(res) => {
                let mut formats = all_formats
                    .iter()
                    .filter(|x| x.resolution == res)
                    .copied()
                    .collect::<Vec<CameraFormat>>();
                formats.sort_by(|a, b| a.frame_rate.cmp(&b.frame_rate));
                let highest_fps = match formats.last() {
                    Some(cf) => cf.frame_rate,
                    None => return None,
                };
                formats
                    .into_iter()
                    .filter(|x| x.frame_rate == highest_fps)
                    .last()
            }
            RequestedFormatType::HighestFrameRate(fps) => {
                let mut formats = all_formats
                    .iter()
                    .filter(|x| x.frame_rate == fps)
                    .copied()
                    .collect::<Vec<CameraFormat>>();
                formats.sort_by(|a, b| a.resolution.cmp(&b.resolution));
                let highest_res = match formats.last() {
                    Some(cf) => cf.resolution,
                    None => return None,
                };
                formats
                    .into_iter()
                    .filter(|x| x.resolution() == highest_res)
                    .last()
            }
            #[allow(clippy::cast_possible_wrap)]
            RequestedFormatType::Closest(c) => {
                let same_fourcc_formats = all_formats
                    .iter()
                    .filter(|x| x.format() == c.format())
                    .copied()
                    .collect::<Vec<CameraFormat>>();
                let mut resolution_map = same_fourcc_formats
                    .iter()
                    .map(|x| {
                        let res = x.resolution();
                        let x_diff = res.x() as i32 - c.resolution().x() as i32;
                        let y_diff = res.y() as i32 - c.resolution().y() as i32;
                        let dist_no_sqrt = (x_diff.abs()).pow(2) + (y_diff.abs()).pow(2);
                        (dist_no_sqrt, res)
                    })
                    .collect::<Vec<(i32, Resolution)>>();
                resolution_map.sort_by(|a, b| a.0.cmp(&b.0));
                resolution_map.dedup_by(|a, b| a.0.eq(&b.0));
                let resolution = resolution_map.first()?.1;

                let frame_rates = all_formats
                    .iter()
                    .filter_map(|camera_format| {
                        if camera_format.format() == c.format() && camera_format.resolution() == c.resolution() {
                            return Some(camera_format.frame_rate());
                        }
                        None
                    })
                    .collect::<Vec<u32>>();
                // sort FPSes
                let mut framerate_map = frame_rates
                    .iter()
                    .map(|x| {
                        let abs = *x as i32 - c.frame_rate() as i32;
                        (abs.unsigned_abs(), *x)
                    })
                    .collect::<Vec<(u32, u32)>>();
                framerate_map.sort_by(|a, b| a.0.cmp(&b.0));
                let frame_rate = framerate_map.first()?.1;
                Some(CameraFormat::new(resolution, c.format(), frame_rate))
            }
            RequestedFormatType::None => all_formats.first().copied(),
        }
    }
}

impl Display for RequestedFormat {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Describes the index of the camera.
/// - Index: A numbered index
/// - String: A string, used for `IPCameras`.
#[derive(Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum CameraIndex {
    Index(u32),
    String(String),
}

impl CameraIndex {
    /// Turns this value into a number. If it is a string, it will attempt to parse it as a `u32`.
    /// # Errors
    /// Fails if the value is not a number.
    pub fn as_index(&self) -> Result<u32, NokhwaError> {
        match self {
            CameraIndex::Index(i) => Ok(*i),
            CameraIndex::String(s) => s
                .parse::<u32>()
                .map_err(|why| NokhwaError::GeneralError(why.to_string())),
        }
    }

    /// Turns this value into a `String`. If it is a number, it will be automatically converted.
    #[must_use]
    pub fn as_string(&self) -> String {
        match self {
            CameraIndex::Index(i) => i.to_string(),
            CameraIndex::String(s) => s.to_string(),
        }
    }

    /// Returns true if this [`CameraIndex`] contains an [`CameraIndex::Index`]
    #[must_use]
    pub fn is_index(&self) -> bool {
        match self {
            CameraIndex::Index(_) => true,
            CameraIndex::String(_) => false,
        }
    }

    /// Returns true if this [`CameraIndex`] contains an [`CameraIndex::String`]
    #[must_use]
    pub fn is_string(&self) -> bool {
        !self.is_index()
    }
}

impl Display for CameraIndex {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_string())
    }
}

impl Default for CameraIndex {
    fn default() -> Self {
        CameraIndex::Index(0)
    }
}

impl TryFrom<CameraIndex> for u32 {
    type Error = NokhwaError;

    fn try_from(value: CameraIndex) -> Result<Self, Self::Error> {
        value.as_index()
    }
}

impl TryFrom<CameraIndex> for usize {
    type Error = NokhwaError;

    fn try_from(value: CameraIndex) -> Result<Self, Self::Error> {
        value.as_index().map(|i| i as usize)
    }
}

/// Describes a Resolution.
/// This struct consists of a Width and a Height value (x,y). <br>
/// Note: the [`Ord`] implementation of this struct is flipped from highest to lowest.
/// # JS-WASM
/// This is exported as `JSResolution`
#[cfg_attr(feature = "output-wasm", wasm_bindgen)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Default, Hash, Eq, PartialEq)]
pub struct Resolution {
    pub width_x: u32,
    pub height_y: u32,
}

#[cfg_attr(feature = "output-wasm", wasm_bindgen)]
impl Resolution {
    /// Create a new resolution from 2 image size coordinates.
    /// # JS-WASM
    /// This is exported as a constructor for [`Resolution`].
    #[must_use]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(constructor))]
    pub fn new(x: u32, y: u32) -> Self {
        Resolution {
            width_x: x,
            height_y: y,
        }
    }

    /// Get the width of Resolution
    /// # JS-WASM
    /// This is exported as `get_Width`.
    #[must_use]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(getter = Width))]
    #[inline]
    pub fn width(self) -> u32 {
        self.width_x
    }

    /// Get the height of Resolution
    /// # JS-WASM
    /// This is exported as `get_Height`.
    #[must_use]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(getter = Height))]
    #[inline]
    pub fn height(self) -> u32 {
        self.height_y
    }

    /// Get the x (width) of Resolution
    #[must_use]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(skip))]
    #[inline]
    pub fn x(self) -> u32 {
        self.width_x
    }

    /// Get the y (height) of Resolution
    #[must_use]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(skip))]
    #[inline]
    pub fn y(self) -> u32 {
        self.height_y
    }
}

impl Display for Resolution {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.x(), self.y())
    }
}

impl PartialOrd for Resolution {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Resolution {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.x().cmp(&other.x()) {
            Ordering::Less => Ordering::Less,
            Ordering::Equal => self.y().cmp(&other.y()),
            Ordering::Greater => Ordering::Greater,
        }
    }
}

/// This is a convenience struct that holds all information about the format of a webcam stream.
/// It consists of a [`Resolution`], [`FrameFormat`], and a frame rate(u8).
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct CameraFormat {
    resolution: Resolution,
    format: FourCC,
    frame_rate: u32,
}

impl CameraFormat {
    /// Construct a new [`CameraFormat`]
    #[must_use]
    pub fn new(resolution: Resolution, format: FourCC, frame_rate: u32) -> Self {
        CameraFormat {
            resolution,
            format,
            frame_rate,
        }
    }

    /// [`CameraFormat::new()`], but raw.
    #[must_use]
    pub fn new_from(res_x: u32, res_y: u32, format: FourCC, fps: u32) -> Self {
        CameraFormat {
            resolution: Resolution {
                width_x: res_x,
                height_y: res_y,
            },
            format,
            frame_rate: fps,
        }
    }

    /// Get the resolution of the current [`CameraFormat`]
    #[must_use]
    pub fn resolution(&self) -> Resolution {
        self.resolution
    }

    /// Get the width of the resolution of the current [`CameraFormat`]
    #[must_use]
    pub fn width(&self) -> u32 {
        self.resolution.width()
    }

    /// Get the height of the resolution of the current [`CameraFormat`]
    #[must_use]
    pub fn height(&self) -> u32 {
        self.resolution.height()
    }

    /// Set the [`CameraFormat`]'s resolution.
    pub fn set_resolution(&mut self, resolution: Resolution) {
        self.resolution = resolution;
    }

    /// Get the frame rate of the current [`CameraFormat`]
    #[must_use]
    pub fn frame_rate(&self) -> u32 {
        self.frame_rate
    }

    /// Set the [`CameraFormat`]'s frame rate.
    pub fn set_frame_rate(&mut self, frame_rate: u32) {
        self.frame_rate = frame_rate;
    }

    /// Get the [`CameraFormat`]'s format.
    #[must_use]
    pub fn format(&self) -> FourCC {
        self.format
    }

    /// Set the [`CameraFormat`]'s format.
    pub fn set_format(&mut self, format: FourCC) {
        self.format = format;
    }
}

impl Default for CameraFormat {
    fn default() -> Self {
        CameraFormat {
            resolution: Resolution::new(640, 480),
            format: MJPEG,
            frame_rate: 30,
        }
    }
}

impl Display for CameraFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}@{}FPS, {} Format",
            self.resolution, self.frame_rate, self.format
        )
    }
}

/// Information about a Camera e.g. its name.
/// `description` amd `misc` may contain information that may differ from backend to backend. Refer to each backend for details.
/// `index` is a camera's index given to it by (usually) the OS usually in the order it is known to the system.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd)]
#[cfg_attr(feature = "output-wasm", wasm_bindgen)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct CameraInfo {
    unique_id: String,
    name: String,
    manufacturer: Option<String>,
    model: Option<String>,
    device_type: Option<String>,
    position: Option<String>,
}

#[cfg_attr(feature = "output-wasm", wasm_bindgen(js_class = CameraInfo))]
impl CameraInfo {
    /// Create a new [`CameraInfo`].
    /// # JS-WASM
    /// This is exported as a constructor for [`CameraInfo`].
    #[must_use]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(constructor))]
    pub fn new(
        unique_id: &str,
        name: &str,
        manufacturer: &str,
        model: &str,
        device_type: &str,
        position: &str,
    ) -> Self {
        CameraInfo {
            unique_id: unique_id.to_string(),
            name: name.to_string(),
            manufacturer: Some(manufacturer.to_string()),
            model: Some(model.to_string()),
            device_type: Some(device_type.to_string()),
            position: Some(position.to_string()),
        }
    }

    /// Get a reference to the device info's human readable name.
    /// # JS-WASM
    /// This is exported as a `get_HumanReadableName`.
    #[must_use]
    #[cfg_attr(
    feature = "output-wasm",
    wasm_bindgen(getter = HumanReadableName)
    )]

    pub fn name(&self) -> String {
        self.name.clone()
    }

    #[must_use]
    pub fn unique_id(&self) -> String {
        self.unique_id.clone()
    }

    #[must_use]
    pub fn manufacturer(&self) -> Option<String> {
        self.manufacturer.clone()
    }

    #[must_use]
    pub fn model(&self) -> Option<String> {
        self.model.clone()
    }

    #[must_use]
    pub fn device_type(&self) -> Option<String> {
        self.device_type.clone()
    }

    #[must_use]
    pub fn position(&self) -> Option<String> {
        self.position.clone()
    }
}

impl Display for CameraInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Name: {} ({}) Manufacturer: {:?}, Model: {:?}, {:?}",
            self.name, self.unique_id, self.manufacturer, self.model, self.position
        )
    }
}

/// The list of known camera controls to the library. <br>
/// These can control the picture brightness, etc. <br>
/// Note that not all backends/devices support all these. Run [`supported_camera_controls()`](crate::traits::CaptureBackendTrait::camera_controls) to see which ones can be set.
#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum KnownCameraControl {
    Brightness,
    Contrast,
    Hue,
    Saturation,
    Sharpness,
    Gamma,
    WhiteBalance,
    BacklightComp,
    Gain,
    Pan,
    Tilt,
    Zoom,
    Exposure,
    Iris,
    Focus,
    /// Other camera control. Listed is the ID.
    /// Wasteful, however is needed for a unified API across Windows, Linux, and `MacOSX` due to Microsoft's usage of GUIDs.
    ///
    /// THIS SHOULD ONLY BE USED WHEN YOU KNOW THE PLATFORM THAT YOU ARE RUNNING ON.
    Other(u128),
}

/// All camera controls in an array.
#[must_use]
pub const fn all_known_camera_controls() -> [KnownCameraControl; 15] {
    [
        KnownCameraControl::Brightness,
        KnownCameraControl::Contrast,
        KnownCameraControl::Hue,
        KnownCameraControl::Saturation,
        KnownCameraControl::Sharpness,
        KnownCameraControl::Gamma,
        KnownCameraControl::WhiteBalance,
        KnownCameraControl::BacklightComp,
        KnownCameraControl::Gain,
        KnownCameraControl::Pan,
        KnownCameraControl::Tilt,
        KnownCameraControl::Zoom,
        KnownCameraControl::Exposure,
        KnownCameraControl::Iris,
        KnownCameraControl::Focus,
    ]
}

impl Display for KnownCameraControl {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", &self)
    }
}

/// This tells you weather a [`KnownCameraControl`] is automatically managed by the OS/Driver
/// or manually managed by you, the programmer.
#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum KnownCameraControlFlag {
    Automatic,
    Manual,
    Continuous,
    ReadOnly,
    WriteOnly,
    Volatile,
    Disabled,
}

impl Display for KnownCameraControlFlag {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// The values for a [`CameraControl`].
///
/// This provides a wide range of values that can be used to control a camera.
#[derive(Clone, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum ControlValueDescription {
    None,
    Integer {
        value: isize,
        default: isize,
        step: isize,
    },
    IntegerRange {
        min: isize,
        max: isize,
        value: isize,
        step: isize,
        default: isize,
    },
    Float {
        value: f64,
        default: f64,
        step: f64,
    },
    FloatRange {
        min: f64,
        max: f64,
        value: f64,
        step: f64,
        default: f64,
    },
    Boolean {
        value: bool,
        default: bool,
    },
    String {
        value: String,
        default: Option<String>,
    },
    Bytes {
        value: Vec<u8>,
        default: Vec<u8>,
    },
    KeyValuePair {
        key: i128,
        value: i128,
        default: (i128, i128),
    },
    Point {
        value: (f64, f64),
        default: (f64, f64),
    },
    Enum {
        value: isize,
        possible: Vec<isize>,
        default: isize,
    },
    RGB {
        value: (f64, f64, f64),
        max: (f64, f64, f64),
        default: (f64, f64, f64),
    },
}

impl ControlValueDescription {
    /// Get the value of this [`ControlValueDescription`]
    #[must_use]
    pub fn value(&self) -> ControlValueSetter {
        match self {
            ControlValueDescription::None => ControlValueSetter::None,
            ControlValueDescription::Integer { value, .. }
            | ControlValueDescription::IntegerRange { value, .. } => {
                ControlValueSetter::Integer(*value)
            }
            ControlValueDescription::Float { value, .. }
            | ControlValueDescription::FloatRange { value, .. } => {
                ControlValueSetter::Float(*value)
            }
            ControlValueDescription::Boolean { value, .. } => ControlValueSetter::Boolean(*value),
            ControlValueDescription::String { value, .. } => {
                ControlValueSetter::String(value.clone())
            }
            ControlValueDescription::Bytes { value, .. } => {
                ControlValueSetter::Bytes(value.clone())
            }
            ControlValueDescription::KeyValuePair { key, value, .. } => {
                ControlValueSetter::KeyValue(*key, *value)
            }
            ControlValueDescription::Point { value, .. } => {
                ControlValueSetter::Point(value.0, value.1)
            }
            ControlValueDescription::Enum { value, .. } => ControlValueSetter::EnumValue(*value),
            ControlValueDescription::RGB { value, .. } => {
                ControlValueSetter::RGB(value.0, value.1, value.2)
            }
        }
    }

    /// Verifies if the [setter](crate::types::ControlValueSetter) is valid for the provided [`ControlValueDescription`].
    /// - `true` => Is valid.
    /// - `false` => Is not valid.
    ///
    /// If the step is 0, it will automatically return `true`.
    #[must_use]
    pub fn verify_setter(&self, setter: &ControlValueSetter) -> bool {
        match self {
            ControlValueDescription::None => setter.as_none().is_some(),
            ControlValueDescription::Integer {
                value,
                default,
                step,
            } => {
                if *step == 0 {
                    return true;
                }
                match setter.as_integer() {
                    Some(i) => (i + default) % step == 0 || (i + value) % step == 0,
                    None => false,
                }
            }
            ControlValueDescription::IntegerRange {
                min,
                max,
                value,
                step,
                default,
            } => {
                if *step == 0 {
                    return true;
                }
                match setter.as_integer() {
                    Some(i) => {
                        ((i + default) % step == 0 || (i + value) % step == 0)
                            && i >= min
                            && i <= max
                    }
                    None => false,
                }
            }
            ControlValueDescription::Float {
                value,
                default,
                step,
            } => {
                if step.abs() == 0_f64 {
                    return true;
                }
                match setter.as_float() {
                    Some(f) => (f - default).abs() % step == 0_f64 || (f - value) % step == 0_f64,
                    None => false,
                }
            }
            ControlValueDescription::FloatRange {
                min,
                max,
                value,
                step,
                default,
            } => {
                if step.abs() == 0_f64 {
                    return true;
                }

                match setter.as_float() {
                    Some(f) => {
                        ((f - default).abs() % step == 0_f64 || (f - value) % step == 0_f64)
                            && f >= min
                            && f <= max
                    }
                    None => false,
                }
            }
            ControlValueDescription::Boolean { .. } => setter.as_boolean().is_some(),
            ControlValueDescription::String { .. } => setter.as_str().is_some(),
            ControlValueDescription::Bytes { .. } => setter.as_bytes().is_some(),
            ControlValueDescription::KeyValuePair { .. } => setter.as_key_value().is_some(),
            ControlValueDescription::Point { .. } => match setter.as_point() {
                Some(pt) => {
                    !pt.0.is_nan() && !pt.1.is_nan() && pt.0.is_finite() && pt.1.is_finite()
                }
                None => false,
            },
            ControlValueDescription::Enum { possible, .. } => match setter.as_enum() {
                Some(e) => possible.contains(e),
                None => false,
            },
            ControlValueDescription::RGB { max, .. } => match setter.as_rgb() {
                Some(v) => *v.0 >= max.0 && *v.1 >= max.1 && *v.2 >= max.2,
                None => false,
            },
        }

        // match setter {
        //     ControlValueSetter::None => {
        //         matches!(self, ControlValueDescription::None)
        //     }
        //     ControlValueSetter::Integer(i) => match self {
        //         ControlValueDescription::Integer {
        //             value,
        //             default,
        //             step,
        //         } => (i - default).abs() % step == 0 || (i - value) % step == 0,
        //         ControlValueDescription::IntegerRange {
        //             min,
        //             max,
        //             value,
        //             step,
        //             default,
        //         } => {
        //             if value > max || value < min {
        //                 return false;
        //             }
        //
        //             (i - default) % step == 0 || (i - value) % step == 0
        //         }
        //         _ => false,
        //     },
        //     ControlValueSetter::Float(f) => match self {
        //         ControlValueDescription::Float {
        //             value,
        //             default,
        //             step,
        //         } => (f - default).abs() % step == 0_f64 || (f - value) % step == 0_f64,
        //         ControlValueDescription::FloatRange {
        //             min,
        //             max,
        //             value,
        //             step,
        //             default,
        //         } => {
        //             if value > max || value < min {
        //                 return false;
        //             }
        //
        //             (f - default) % step == 0_f64 || (f - value) % step == 0_f64
        //         }
        //         _ => false,
        //     },
        //     ControlValueSetter::Boolean(b) => {
        //
        //     }
        //     ControlValueSetter::String(_) => {
        //         matches!(self, ControlValueDescription::String { .. })
        //     }
        //     ControlValueSetter::Bytes(_) => {
        //         matches!(self, ControlValueDescription::Bytes { .. })
        //     }
        //     ControlValueSetter::KeyValue(_, _) => {
        //         matches!(self, ControlValueDescription::KeyValuePair { .. })
        //     }
        //     ControlValueSetter::Point(_, _) => {
        //         matches!(self, ControlValueDescription::Point { .. })
        //     }
        //     ControlValueSetter::EnumValue(_) => {
        //         matches!(self, ControlValueDescription::Enum { .. })
        //     }
        //     ControlValueSetter::RGB(_, _, _) => {
        //         matches!(self, ControlValueDescription::RGB { .. })
        //     }
        // }
    }
}

impl Display for ControlValueDescription {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ControlValueDescription::None => {
                write!(f, "(None)")
            }
            ControlValueDescription::Integer {
                value,
                default,
                step,
            } => {
                write!(f, "(Current: {value}, Default: {default}, Step: {step})",)
            }
            ControlValueDescription::IntegerRange {
                min,
                max,
                value,
                step,
                default,
            } => {
                write!(
                    f,
                    "(Current: {value}, Default: {default}, Step: {step}, Range: ({min}, {max}))",
                )
            }
            ControlValueDescription::Float {
                value,
                default,
                step,
            } => {
                write!(f, "(Current: {value}, Default: {default}, Step: {step})",)
            }
            ControlValueDescription::FloatRange {
                min,
                max,
                value,
                step,
                default,
            } => {
                write!(
                    f,
                    "(Current: {value}, Default: {default}, Step: {step}, Range: ({min}, {max}))",
                )
            }
            ControlValueDescription::Boolean { value, default } => {
                write!(f, "(Current: {value}, Default: {default})")
            }
            ControlValueDescription::String { value, default } => {
                write!(f, "(Current: {value}, Default: {default:?})")
            }
            ControlValueDescription::Bytes { value, default } => {
                write!(f, "(Current: {value:x?}, Default: {default:x?})")
            }
            ControlValueDescription::KeyValuePair {
                key,
                value,
                default,
            } => {
                write!(
                    f,
                    "Current: ({key}, {value}), Default: ({}, {})",
                    default.0, default.1
                )
            }
            ControlValueDescription::Point { value, default } => {
                write!(
                    f,
                    "Current: ({}, {}), Default: ({}, {})",
                    value.0, value.1, default.0, default.1
                )
            }
            ControlValueDescription::Enum {
                value,
                possible,
                default,
            } => {
                write!(
                    f,
                    "Current: {value}, Possible Values: {possible:?}, Default: {default}",
                )
            }
            ControlValueDescription::RGB {
                value,
                max,
                default,
            } => {
                write!(
                    f,
                    "Current: ({}, {}, {}), Max: ({}, {}, {}), Default: ({}, {}, {})",
                    value.0, value.1, value.2, max.0, max.1, max.2, default.0, default.1, default.2
                )
            }
        }
    }
}

/// This struct tells you everything about a particular [`KnownCameraControl`].
///
/// However, you should never need to instantiate this struct, since its usually generated for you by `jaenokhwa`.
/// The only time you should be modifying this struct is when you need to set a value and pass it back to the camera.
/// NOTE: Assume the values for `min` and `max` as **non-inclusive**!.
/// E.g. if the [`CameraControl`] says `min` is 100, the minimum is actually 101.
#[derive(Clone, Debug, PartialOrd, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct CameraControl {
    control: KnownCameraControl,
    name: String,
    description: ControlValueDescription,
    flag: Vec<KnownCameraControlFlag>,
    active: bool,
}

impl CameraControl {
    /// Creates a new [`CameraControl`]
    #[must_use]
    pub fn new(
        control: KnownCameraControl,
        name: String,
        description: ControlValueDescription,
        flag: Vec<KnownCameraControlFlag>,
        active: bool,
    ) -> Self {
        CameraControl {
            control,
            name,
            description,
            flag,
            active,
        }
    }

    /// Gets the name of this [`CameraControl`]
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Gets the [`ControlValueDescription`] of this [`CameraControl`]
    #[must_use]
    pub fn description(&self) -> &ControlValueDescription {
        &self.description
    }

    /// Gets the [`ControlValueSetter`] of the [`ControlValueDescription`] of this [`CameraControl`]
    #[must_use]
    pub fn value(&self) -> ControlValueSetter {
        self.description.value()
    }

    /// Gets the [`KnownCameraControl`] of this [`CameraControl`]
    #[must_use]
    pub fn control(&self) -> KnownCameraControl {
        self.control
    }

    /// Gets the [`KnownCameraControlFlag`] of this [`CameraControl`],
    /// telling you weather this control is automatically set or manually set.
    #[must_use]
    pub fn flag(&self) -> &[KnownCameraControlFlag] {
        &self.flag
    }

    /// Gets `active` of this [`CameraControl`],
    /// telling you weather this control is currently active(in-use).
    #[must_use]
    pub fn active(&self) -> bool {
        self.active
    }

    /// Gets `active` of this [`CameraControl`],
    /// telling you weather this control is currently active(in-use).
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }
}

impl Display for CameraControl {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Control: {}, Name: {}, Value: {}, Flag: {:?}, Active: {}",
            self.control, self.name, self.description, self.flag, self.active
        )
    }
}

/// The setter for a control value
#[derive(Clone, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum ControlValueSetter {
    None,
    Integer(isize),
    Float(f64),
    Boolean(bool),
    String(String),
    Bytes(Vec<u8>),
    KeyValue(i128, i128),
    Point(f64, f64),
    EnumValue(isize),
    RGB(f64, f64, f64),
}

impl ControlValueSetter {
    #[must_use]
    pub fn as_none(&self) -> Option<()> {
        if let ControlValueSetter::None = self {
            Some(())
        } else {
            None
        }
    }
    #[must_use]

    pub fn as_integer(&self) -> Option<&isize> {
        if let ControlValueSetter::Integer(i) = self {
            Some(i)
        } else {
            None
        }
    }
    #[must_use]

    pub fn as_float(&self) -> Option<&f64> {
        if let ControlValueSetter::Float(f) = self {
            Some(f)
        } else {
            None
        }
    }
    #[must_use]

    pub fn as_boolean(&self) -> Option<&bool> {
        if let ControlValueSetter::Boolean(f) = self {
            Some(f)
        } else {
            None
        }
    }
    #[must_use]

    pub fn as_str(&self) -> Option<&str> {
        if let ControlValueSetter::String(s) = self {
            Some(s)
        } else {
            None
        }
    }
    #[must_use]

    pub fn as_bytes(&self) -> Option<&[u8]> {
        if let ControlValueSetter::Bytes(b) = self {
            Some(b)
        } else {
            None
        }
    }
    #[must_use]

    pub fn as_key_value(&self) -> Option<(&i128, &i128)> {
        if let ControlValueSetter::KeyValue(k, v) = self {
            Some((k, v))
        } else {
            None
        }
    }
    #[must_use]

    pub fn as_point(&self) -> Option<(&f64, &f64)> {
        if let ControlValueSetter::Point(x, y) = self {
            Some((x, y))
        } else {
            None
        }
    }
    #[must_use]

    pub fn as_enum(&self) -> Option<&isize> {
        if let ControlValueSetter::EnumValue(e) = self {
            Some(e)
        } else {
            None
        }
    }
    #[must_use]

    pub fn as_rgb(&self) -> Option<(&f64, &f64, &f64)> {
        if let ControlValueSetter::RGB(r, g, b) = self {
            Some((r, g, b))
        } else {
            None
        }
    }
}

impl Display for ControlValueSetter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ControlValueSetter::None => {
                write!(f, "Value: None")
            }
            ControlValueSetter::Integer(i) => {
                write!(f, "IntegerValue: {i}")
            }
            ControlValueSetter::Float(d) => {
                write!(f, "FloatValue: {d}")
            }
            ControlValueSetter::Boolean(b) => {
                write!(f, "BoolValue: {b}")
            }
            ControlValueSetter::String(s) => {
                write!(f, "StrValue: {s}")
            }
            ControlValueSetter::Bytes(b) => {
                write!(f, "BytesValue: {b:x?}")
            }
            ControlValueSetter::KeyValue(k, v) => {
                write!(f, "KVValue: ({k}, {v})")
            }
            ControlValueSetter::Point(x, y) => {
                write!(f, "PointValue: ({x}, {y})")
            }
            ControlValueSetter::EnumValue(v) => {
                write!(f, "EnumValue: {v}")
            }
            ControlValueSetter::RGB(r, g, b) => {
                write!(f, "RGBValue: ({r}, {g}, {b})")
            }
        }
    }
}

/// The list of known capture backends to the library. <br>
/// - `AUTO` is special - it tells the Camera struct to automatically choose a backend most suited for the current platform.
/// - `AVFoundation` - Uses `AVFoundation` on `MacOSX`
/// - `Video4Linux` - `Video4Linux2`, a linux specific backend.
/// - `UniversalVideoClass` -  ***DEPRECATED*** Universal Video Class (please check [libuvc](https://github.com/libuvc/libuvc)). Platform agnostic, although on linux it needs `sudo` permissions or similar to use.
/// - `MediaFoundation` - Microsoft Media Foundation, Windows only,
/// - `GStreamer` - ***DEPRECATED*** Uses `GStreamer` RTP to capture. Platform agnostic.
/// - `Browser` - Uses browser APIs to capture from a webcam.
#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum ApiBackend {
    Auto,
    AVFoundation,
    Video4Linux,
    MediaFoundation,
    Browser,
}

impl Display for ApiBackend {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
