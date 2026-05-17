/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Abstraction of window setup, OpenGL context creation and event handling.
//!
//! Implemented using the sdl2 crate (a Rust wrapper for SDL2). All usage of
//! SDL should be confined to this module.
//!
//! There is currently no separation of concerns between a single window and
//! window system interaction in general, because it is assumed only one window
//! will be needed for the runtime of the app.

use crate::gles::present::present_frame;
use crate::gles::{create_gles1_ctx_no_parent_stack, GLESContext, GLES};
use crate::image::Image;
use crate::matrix::Matrix;
use crate::options::Options;
use crate::Environment;
use sdl2::mouse::MouseButton;
use sdl2::pixels::PixelFormatEnum;
use sdl2::surface::Surface;
use sdl2_sys::SDL_PowerState;
use std::collections::{HashMap, VecDeque};
use std::env;
use std::f32::consts::FRAC_PI_2;
use std::num::NonZeroU32;
use std::ptr::null_mut;
use std::time::{Duration, Instant};

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DeviceFamily {
    iPhone,
    iPhone5,
    iPad,
}
impl std::fmt::Display for DeviceFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}
impl DeviceFamily {
    /// Portrait (width, height) in logical points.
    /// iPhone 5 screen is 1136×640 px retina (2×), so 568×320 pts.
    pub fn portrait_size(&self) -> (u32, u32) {
        match self {
            DeviceFamily::iPhone => (320, 480),
            DeviceFamily::iPhone5 => (320, 568),
            DeviceFamily::iPad => (768, 1024),
        }
    }
    /// UIScreen.scale — retina multiplier.
    pub fn scale_factor(&self) -> f32 {
        match self {
            DeviceFamily::iPhone => 1.0,
            DeviceFamily::iPhone5 => 2.0,
            DeviceFamily::iPad => 1.0,
        }
    }
    /// hw.machine string returned by sysctl / uname.
    pub fn machine_name(&self) -> &'static str {
        match self {
            DeviceFamily::iPhone => "iPhone1,1",
            DeviceFamily::iPhone5 => "iPhone5,1",
            DeviceFamily::iPad => "iPad1,1",
        }
    }
}
impl TryFrom<u64> for DeviceFamily {
    type Error = ();
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(DeviceFamily::iPhone),
            2 => Ok(DeviceFamily::iPad),
            _ => Err(()),
        }
    }
}
impl TryFrom<&str> for DeviceFamily {
    type Error = ();
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "iphone" => Ok(DeviceFamily::iPhone),
            "iphone5" => Ok(DeviceFamily::iPhone5),
            "ipad" => Ok(DeviceFamily::iPad),
            _ => Err(()),
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DeviceOrientation {
    Portrait,
    LandscapeLeft,
    LandscapeRight,
}
fn size_for_orientation(
    family: DeviceFamily,
    orientation: DeviceOrientation,
    scale_hack: NonZeroU32,
) -> (u32, u32) {
    let (width, height) = family.portrait_size();
    let scale_hack = scale_hack.get();
    match orientation {
        DeviceOrientation::Portrait => (width * scale_hack, height * scale_hack),
        DeviceOrientation::LandscapeLeft => (height * scale_hack, width * scale_hack),
        DeviceOrientation::LandscapeRight => (height * scale_hack, width * scale_hack),
    }
}
fn rotate_fullscreen_size(orientation: DeviceOrientation, screen_size: (u32, u32)) -> (u32, u32) {
    let (short_side, long_side) = if screen_size.0 < screen_size.1 {
        (screen_size.0, screen_size.1)
    } else {
        (screen_size.1, screen_size.0)
    };
    match orientation {
        DeviceOrientation::Portrait => (short_side, long_side),
        DeviceOrientation::LandscapeLeft | DeviceOrientation::LandscapeRight => {
            (long_side, short_side)
        }
    }
}
/// Tell SDL2 what orientation we want. Only useful on Android.
fn set_sdl2_orientation(orientation: DeviceOrientation) {
    // Despite the name, this hint works on Android too.
    sdl2::hint::set(
        "SDL_IOS_ORIENTATIONS",
        match orientation {
            DeviceOrientation::Portrait => "Portrait",
            // The inversion is deliberate. These probably correspond to
            // iPhone OS content orientations?
            DeviceOrientation::LandscapeLeft => "LandscapeRight",
            DeviceOrientation::LandscapeRight => "LandscapeLeft",
        },
    );
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum FingerId {
    Mouse,
    Touch(i64),
    VirtualCursor,
    ButtonToTouch(crate::options::Button),
    StickToTouch,
    DpadToTouch,
}
pub type Coords = (f32, f32);

struct DpadState {
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    active: bool,
}

#[derive(Debug)]
pub enum TextInputEvent {
    Text(String),
    Backspace,
    Return,
}

#[derive(Debug)]
pub enum Event {
    Quit,
    TouchesDown(std::collections::HashMap<FingerId, Coords>),
    TouchesMove(std::collections::HashMap<FingerId, Coords>),
    TouchesUp(std::collections::HashMap<FingerId, Coords>),
    TouchesCancel(std::collections::HashMap<FingerId, Coords>), 
    AppWillResignActive,
    AppWillTerminate,
    EnterDebugger,
    TextInput(TextInputEvent),
}

pub enum BatteryState {
    Unknown,
    OnBattery,
    NoBattery,
    Charging,
    Full,
}

pub enum GLVersion {
    /// OpenGL ES 1.1
    GLES11,
    /// OpenGL ES 2.0
    GLES20,
    /// OpenGL 2.1 compatibility profile
    GL21Compat,
}

pub struct GLContext(sdl2::video::GLContext);

impl GLContext {
    pub fn is_current(&self) -> bool {
        self.0.is_current()
    }
}

fn surface_from_image(image: &Image) -> Surface<'_> {
    let src_pixels = image.pixels();
    let (width, height) = image.dimensions();

    let mut surface = Surface::new(width, height, PixelFormatEnum::RGBA32).unwrap();
    let (width, height) = (width as usize, height as usize);
    let pitch = surface.pitch() as usize;
    surface.with_lock_mut(|dst_pixels| {
        for y in 0..height {
            for x in 0..width {
                for channel in 0..4 {
                    let src_idx = y * width * 4 + x * 4 + channel;
                    let dst_idx = y * pitch + x * 4 + channel;
                    dst_pixels[dst_idx] = src_pixels[src_idx];
                }
            }
        }
    });
    surface
}

pub struct Window {
    _sdl_ctx: sdl2::Sdl,
    video_ctx: sdl2::VideoSubsystem,
    window: sdl2::video::Window,
    event_pump: sdl2::EventPump,
    event_queue: VecDeque<Event>,
    last_polled: Instant,
    /// Separate queue for extremely high-priority events (e.g. app about to
    /// terminate).
    high_priority_event: Option<Event>,
    enable_event_polling: bool,
    #[cfg(target_os = "macos")]
    max_height: u32,
    #[cfg(target_os = "macos")]
    viewport_y_offset: u32,
    /// Copy of `fullscreen` on [Options]. Note that this is meaningless when
    /// [Self::rotatable_fullscreen] returns [true].
    fullscreen: bool,
    scale_hack: NonZeroU32,
    internal_gl_ins: Option<Box<dyn GLESContext>>,
    splash_image: Option<Image>,
    device_family: DeviceFamily,
    device_orientation: DeviceOrientation,
    controller_ctx: sdl2::GameControllerSubsystem,
    controllers: Vec<sdl2::controller::GameController>,
    dpad_state: DpadState,
    stick_active: bool,
    _sensor_ctx: sdl2::SensorSubsystem,
    accelerometer: Option<sdl2::sensor::Sensor>,
    virtual_cursor_last: Option<(f32, f32, bool, bool)>,
    virtual_cursor_last_unsticky: Option<(f32, f32, Instant)>,
    virtual_accelerometer_last: Option<(f32, f32, bool)>,
    /// Whether or not we are on the "main" environment stack (rather than
    /// a coroutine stack). Checked in various functions to make sure that
    /// certain SDL functions (that call JNI functions) are on the main
    /// stack on Android.
    pub(super) on_main_stack: bool,
}

impl Window {
    /// Returns [true] if touchHLE is running on a device where we should always
    /// display fullscreen, but SDL2 will let us control the orientation, i.e.
    /// Android devices.
    pub fn rotatable_fullscreen() -> bool {
        env::consts::OS == "android"
    }
    pub fn new(
        title: &str,
        icon: Option<Image>,
        launch_image: Option<Image>,
        options: &Options,
    ) -> Window {
        let sdl_ctx = sdl2::init().unwrap();
        let video_ctx = sdl_ctx.video().unwrap();

        // The "hidapi" feature of rust-sdl2 is enabled so that sdl2::sensor
        // is available, but we don't want to enable SDL's HIDAPI controller
        // drivers because they cause duplicated controllers on macOS
        // (https://github.com/libsdl-org/SDL/issues/7479). Once that's fixed,
        // remove this (https://github.com/touchHLE/touchHLE/issues/85).
        sdl2::hint::set("SDL_JOYSTICK_HIDAPI", "0");

        if env::consts::OS == "android" {
            // It's important to set context version BEFORE window creation
            // ref. https://wiki.libsdl.org/SDL2/SDL_GLattr
            let attr = video_ctx.gl_attr();
            attr.set_context_version(1, 1);
            attr.set_context_profile(sdl2::video::GLProfile::GLES);

            // Disable blocking of event loop when app is paused.
            sdl2::hint::set("SDL_ANDROID_BLOCK_ON_PAUSE", "0");
        }

        // Separate mouse and touch events
        sdl2::hint::set("SDL_TOUCH_MOUSE_EVENTS", "0");

        // SDL2 disables the screen saver by default, but iPhone OS enables
        // the idle timer that triggers sleep by default, so we turn it back on
        // here, and then the app can disable it if it wants to.
        video_ctx.enable_screen_saver();

        let scale_hack = options.scale_hack;
        // TODO: some apps specify their orientation in Info.plist, we could use
        // that here.
        let device_family = options.device_family.unwrap_or(DeviceFamily::iPhone);
        let device_orientation = options.initial_orientation;
        let fullscreen = options.fullscreen;

        let mut window = if Self::rotatable_fullscreen() {
            // Without this, SDL will force fullscreen mode to be portrait.
            set_sdl2_orientation(device_orientation);
            let screen_size = video_ctx.display_bounds(0).unwrap().size();
            let (width, height) = rotate_fullscreen_size(device_orientation, screen_size);
            let window = video_ctx
                .window(title, width, height)
                .fullscreen()
                .opengl()
                .build()
                .unwrap();
            window
        } else if fullscreen {
            let (width, height) = video_ctx.display_bounds(0).unwrap().size();
            let window = video_ctx
                .window(title, width, height)
                .fullscreen_desktop()
                .opengl()
                .build()
                .unwrap();
            window
        } else {
            let (width, height) =
                size_for_orientation(device_family, device_orientation, scale_hack);
            let window = video_ctx
                .window(title, width, height)
                .position_centered()
                .opengl()
                .build()
                .unwrap();
            window
        };

        if env::consts::OS == "android" {
            // Sanity check
            let gl_attr = video_ctx.gl_attr();
            debug_assert_eq!(gl_attr.context_profile(), sdl2::video::GLProfile::GLES);
            debug_assert_eq!(gl_attr.context_version(), (1, 1));
        }

        if let Some(icon) = icon {
            window.set_icon(surface_from_image(&icon));
        }

        let event_pump = sdl_ctx.event_pump().unwrap();

        let controller_ctx = sdl_ctx.game_controller().unwrap();

        let sensor_ctx = sdl_ctx.sensor().unwrap();
        let mut accelerometer: Option<sdl2::sensor::Sensor> = None;
        if let Ok(num_sensors) = sensor_ctx.num_sensors() {
            for sensor_idx in 0..num_sensors {
                if let Ok(sensor) = sensor_ctx.open(sensor_idx) {
                    if sensor.sensor_type() == sdl2::sensor::SensorType::Accelerometer {
                        log!("Accelerometer detected: {}.", sensor.name());
                        accelerometer = Some(sensor);
                        break;
                    }
                }
            }
        }

        #[cfg(target_os = "macos")]
        let max_height = window.size().1;

        let mut window = Window {
            _sdl_ctx: sdl_ctx,
            video_ctx,
            window,
            event_pump,
            event_queue: VecDeque::new(),
            last_polled: Instant::now() - Duration::from_secs(1),
            high_priority_event: None,
            enable_event_polling: true,
            #[cfg(target_os = "macos")]
            max_height,
            #[cfg(target_os = "macos")]
            viewport_y_offset: 0,
            fullscreen,
            scale_hack,
            internal_gl_ins: None,
            splash_image: launch_image,
            device_family,
            device_orientation,
            controller_ctx,
            controllers: Vec::new(),
            dpad_state: DpadState {
                left: false,
                right: false,
                up: false,
                down: false,
                active: false,
            },
            stick_active: false,
            _sensor_ctx: sensor_ctx,
            accelerometer,
            virtual_cursor_last: None,
            virtual_cursor_last_unsticky: None,
            virtual_accelerometer_last: None,
            on_main_stack: true,
        };

        // Set up OpenGL ES context used for splash screen and app UI rendering
        // (see src/frameworks/core_animation/composition.rs). OpenGL ES is used
        // because SDL2 won't let us use more than one graphics API in the same
        // window, and we also need OpenGL ES for the app's own rendering.
        let mut gl_ins = create_gles1_ctx_no_parent_stack(&mut window, options);
        {
            let gl_ctx = gl_ins.make_current(&mut window);
            log!("Driver info: {}", unsafe { gl_ctx.driver_description() });
        }
        window.internal_gl_ins = Some(gl_ins);

        if window.splash_image.is_some() {
            window.display_splash();
        }

        window
    }

    pub fn poll_for_events(&mut self, options: &Options) {
        if !self.on_main_stack {
            log!("Warning: poll_for_events called off main stack, skipping");
            return;
        }
        let now = Instant::now();
        if now.duration_since(self.last_polled) < Duration::from_secs_f64(1.0 / 120.0) {
            return;
        }
        self.last_polled = now;

        fn transform_input_coords(
            window: &Window,
            (in_x, in_y): (f32, f32),
            independent_of_viewport: bool,
        ) -> (f32, f32) {
            let (vx, vy, vw, vh) = if independent_of_viewport {
                let (width, height) = size_for_orientation(
                    window.device_family,
                    window.device_orientation,
                    NonZeroU32::new(1).unwrap(),
                );
                (0, 0, width, height)
            } else {
                window.viewport()
            };
            let in_x = in_x.clamp(vx as f32, (vx + vw) as f32);
            let in_y = in_y.clamp(vy as f32, (vy + vh) as f32);
            let x = (in_x - vx as f32) / vw as f32 - 0.5;
            let y = (in_y - vy as f32) / vh as f32 - 0.5;
            let matrix = window.rotation_matrix().inverse().unwrap();
            let [x, y] = matrix.transform([x, y]);
            let (out_w, out_h) = window.size_unrotated_unscaled();
            let out_x = (x + 0.5) * out_w as f32;
            let out_y = (y + 0.5) * out_h as f32;
            let max_x = (out_w.saturating_sub(1)) as f32;
            let max_y = (out_h.saturating_sub(1)) as f32;
            let out_x = out_x.clamp(0.0, max_x);
            let out_y = out_y.clamp(0.0, max_y);
            (out_x.round(), out_y.round())
        }
        fn transform_virt_accel_coords(window: &Window, (in_x, in_y): (i32, i32)) -> (f32, f32) {
            let (_, _, vw, vh) = window.viewport();
            let out_x = ((in_x as f32 / vw as f32) * 2.0 - 1.0).clamp(-1.0, 1.0);
            let out_y = ((in_y as f32 / vh as f32) * 2.0 - 1.0).clamp(-1.0, 1.0);
            (out_x, out_y)
        }
        fn translate_button(button: sdl2::controller::Button) -> Option<crate::options::Button> {
            match button {
                sdl2::controller::Button::DPadLeft => Some(crate::options::Button::DPadLeft),
                sdl2::controller::Button::DPadUp => Some(crate::options::Button::DPadUp),
                sdl2::controller::Button::DPadRight => Some(crate::options::Button::DPadRight),
                sdl2::controller::Button::DPadDown => Some(crate::options::Button::DPadDown),
                sdl2::controller::Button::Start => Some(crate::options::Button::Start),
                sdl2::controller::Button::A => Some(crate::options::Button::A),
                sdl2::controller::Button::B => Some(crate::options::Button::B),
                sdl2::controller::Button::X => Some(crate::options::Button::X),
                sdl2::controller::Button::Y => Some(crate::options::Button::Y),
                sdl2::controller::Button::LeftShoulder => {
                    Some(crate::options::Button::LeftShoulder)
                }
                _ => None,
            }
        }
        fn finger_absolute_coords(window: &Window, (x, y): (f32, f32)) -> (f32, f32) {
            let (screen_width, screen_height) = window.window.drawable_size();
            (screen_width as f32 * x, screen_height as f32 * y)
        }

        let mut controller_updated = false;
        let mut previous_event: Option<sdl2::event::Event> = None;
        while self.enable_event_polling {
            use sdl2::event::Event as E;
            let event = if let Some(e) = previous_event.take() {
                e
            } else if let Some(e) = self.event_pump.poll_event() {
                e
            } else {
                break;
            };

            // Virtual accelerometer
            match event {
                E::MouseButtonDown {
                    x,
                    y,
                    mouse_btn: MouseButton::Right,
                    ..
                } => {
                    let (x, y) = transform_virt_accel_coords(self, (x, y));
                    self.virtual_accelerometer_last = Some((x, y, true));
                }
                E::MouseMotion {
                    x, y, mousestate, ..
                } => {
                    if mousestate.right() {
                        let (x, y) = transform_virt_accel_coords(self, (x, y));
                        self.virtual_accelerometer_last = Some((x, y, true));
                    }
                }
                E::MouseButtonUp {
                    x,
                    y,
                    mouse_btn: MouseButton::Right,
                    ..
                } => {
                    let (x, y) = transform_virt_accel_coords(self, (x, y));
                    self.virtual_accelerometer_last = Some((x, y, false));
                }
                _ => {}
            }

            self.event_queue.push_back(match event {
                E::Quit { .. } => Event::Quit,
                E::MouseButtonDown {
                    x,
                    y,
                    mouse_btn: MouseButton::Left,
                    ..
                } => {
                    let coords = transform_input_coords(self, (x as f32, y as f32), false);
                    Event::TouchesDown(HashMap::from([(FingerId::Mouse, coords)]))
                    }
                E::MouseMotion {
                    x, y, mousestate, ..
                } if mousestate.left() => {
                    let coords = transform_input_coords(self, (x as f32, y as f32), false);
                    Event::TouchesMove(HashMap::from([(FingerId::Mouse, coords)]))
                }
                E::MouseButtonUp {
                    x,
                    y,
                    mouse_btn: MouseButton::Left,
                    ..
                } => {
                    let coords = transform_input_coords(self, (x as f32, y as f32), false);
                    Event::TouchesUp(HashMap::from([(FingerId::Mouse, coords)]))
                }
                E::ControllerDeviceAdded { which, .. } => {
                    self.controller_added(which);
                    continue;
                }
                E::ControllerDeviceRemoved { which, .. } => {
                    self.controller_removed(which);
                    continue;
                }
                E::ControllerButtonUp { button, .. } | E::ControllerButtonDown { button, .. } => {
                    controller_updated = true;
                    let Some(button) = translate_button(button) else {
                        continue;
                    };
                    if (button == crate::options::Button::DPadLeft
                        || button == crate::options::Button::DPadUp
                        || button == crate::options::Button::DPadRight
                        || button == crate::options::Button::DPadDown)
                        && options.dpad_to_touch.is_some()
                    {
                        let Some((x, y, w, h)) = options.dpad_to_touch else {
                            unreachable!();
                        };

                        let pressed = matches!(event, E::ControllerButtonDown { .. });
                        match button {
                            crate::options::Button::DPadLeft => self.dpad_state.left = pressed,
                            crate::options::Button::DPadRight => self.dpad_state.right = pressed,
                            crate::options::Button::DPadUp => self.dpad_state.up = pressed,
                            crate::options::Button::DPadDown => self.dpad_state.down = pressed,
                            _ => unreachable!(),
                        }

                        let cx = x + w * 0.5;
                        let cy = y + h * 0.5;
                        let mut dx = 0.0;
                        let mut dy = 0.0;

                        if self.dpad_state.left {
                            dx -= 0.5 * w;
                        }
                        if self.dpad_state.right {
                            dx += 0.5 * w;
                        }
                        if self.dpad_state.up {
                            dy -= 0.5 * h;
                        }
                        if self.dpad_state.down {
                            dy += 0.5 * h;
                        }

                        let coords = transform_input_coords(self, (cx + dx, cy + dy), true);
                        let any_held = self.dpad_state.left
                            || self.dpad_state.right
                            || self.dpad_state.up
                            || self.dpad_state.down;

                        if !self.dpad_state.active && any_held {
                            self.dpad_state.active = true;
                            Event::TouchesDown(HashMap::from([(FingerId::DpadToTouch, coords)]))
                        } else if self.dpad_state.active && any_held {
                            Event::TouchesMove(HashMap::from([(FingerId::DpadToTouch, coords)]))
                        } else if self.dpad_state.active && !any_held {
                            self.dpad_state.active = false;
                            Event::TouchesUp(HashMap::from([(FingerId::DpadToTouch, coords)]))
                        } else {
                            continue;
                        }
                    } else {
                        let Some(&(x, y)) = options.button_to_touch.get(&button) else {
                            continue;
                        };
                        match event {
                            E::ControllerButtonUp { .. } => {
                                let coords = transform_input_coords(self, (x, y), true);
                                Event::TouchesUp(HashMap::from([(
                                    FingerId::ButtonToTouch(button),
                                    coords,
                                )]))
                            }
                            E::ControllerButtonDown { .. } => {
                                let coords = transform_input_coords(self, (x, y), true);
                                Event::TouchesDown(HashMap::from([(
                                    FingerId::ButtonToTouch(button),
                                    coords,
                                )]))
                            }
                            _ => unreachable!(),
                        }
                    }
                }
                E::ControllerAxisMotion { axis, .. } => {
                    controller_updated = true;
                    let Some((x, y, w, h)) = options.stick_to_touch else {
                        continue;
                    };
                    if axis == sdl2::controller::Axis::LeftX
                        || axis == sdl2::controller::Axis::LeftY
                    {
                        let (stick_x, stick_y, _) = self.get_controller_stick(options, true);
                        let coords = transform_input_coords(
                            self,
                            (
                                x + ((stick_x + 1.0) / 2.0) * w,
                                y + ((stick_y + 1.0) / 2.0) * h,
                            ),
                            true,
                        );
                        if stick_x.abs() < options.deadzone && stick_y.abs() < options.deadzone {
                            if !self.stick_active {
                                continue;
                            } else {
                                self.stick_active = false;
                                Event::TouchesUp(HashMap::from([(FingerId::StickToTouch, coords)]))
                            }
                        } else if !self.stick_active {
                            self.stick_active = true;
                            Event::TouchesDown(HashMap::from([(FingerId::StickToTouch, coords)]))
                        } else {
                            Event::TouchesMove(HashMap::from([(FingerId::StickToTouch, coords)]))
                        }
                    } else {
                        continue;
                    }
                }
                E::AppWillEnterBackground { .. } => {
                    // FIX: Log the warning message but completely skip injecting 
                    // Event::AppWillResignActive and do NOT set self.enable_event_polling to false.
                    // This strips the host-side focus drop signal from freezing the emulation state.
                    log!("Intercepted host focus event: ignoring AppWillEnterBackground to prevent freeze.");
                    continue;
                }
                E::AppTerminating { .. } => {
                    log!("Received app-will-terminate event.");
                    assert!(self.high_priority_event.is_none());
                    self.high_priority_event = Some(Event::AppWillTerminate);
                    self.enable_event_polling = false;
                    continue;
                }
                E::FingerUp {
                    timestamp,
                    finger_id,
                    x,
                    y,
                    ..
                }
                | E::FingerMotion {
                    timestamp,
                    finger_id,
                    x,
                    y,
                    ..
                }
                | E::FingerDown {
                    timestamp,
                    finger_id,
                    x,
                    y,
                    ..
                } => {
                    let curr_timestamp = timestamp;
                    let abs_coords = finger_absolute_coords(self, (x, y));
                    let coords = transform_input_coords(self, abs_coords, false);
                    let mut map = HashMap::from([(FingerId::Touch(finger_id), coords)]);
                    while let Some(next) = self.event_pump.poll_event() {
                        match next {
                            E::FingerUp {
                                timestamp,
                                finger_id,
                                x,
                                y,
                                ..
                            }
                            | E::FingerMotion {
                                timestamp,
                                finger_id,
                                x,
                                y,
                                ..
                            }
                            | E::FingerDown {
                                timestamp,
                                finger_id,
                                x,
                                y,
                                ..
                            } if timestamp == curr_timestamp && next.is_same_kind_as(&event) => {
                                let abs_coords = finger_absolute_coords(self, (x, y));
                                let coords = transform_input_coords(self, abs_coords, false);
                                map.insert(FingerId::Touch(finger_id), coords);
                            }
                            E::MultiGesture { timestamp, .. } if timestamp == curr_timestamp => {
                                continue;
                            }
                            _ => {
                                assert!(previous_event.is_none());
                                previous_event = Some(next);
                                break;
                            }
                        }
                    }
                    match event {
                        E::FingerUp { .. } => Event::TouchesUp(map),
                        E::FingerMotion { .. } => Event::TouchesMove(map),
                        E::FingerDown { .. } => Event::TouchesDown(map),
                        _ => unreachable!(),
                    }
                }
                E::KeyDown {
                    keycode: Some(sdl2::keyboard::Keycode::F12),
                    ..
                } => {
                    echo!("F12 pressed, EnterDebugger event queued.");
                    Event::EnterDebugger
                }
                E::KeyDown {
                    keycode: Some(sdl2::keyboard::Keycode::Backspace),
                    ..
                } => {
                    Event::TextInput(TextInputEvent::Backspace)
                }
                E::KeyDown {
                    keycode: Some(sdl2::keyboard::Keycode::Return),
                    ..
                } => {
                    Event::TextInput(TextInputEvent::Return)
                }
                E::TextInput { text, .. } => {
                    Event::TextInput(TextInputEvent::Text(text))
                }
                _ => continue,
            })
        }

        if controller_updated {
            let (new_x, new_y, pressed, pressed_changed, moved) =
                self.update_virtual_cursor(options);
            self.event_queue
                .push_back(match (pressed, pressed_changed, moved) {
                    (true, true, _) => {
                        let coords = transform_input_coords(self, (new_x, new_y), false);
                        Event::TouchesDown(HashMap::from([(FingerId::VirtualCursor, coords)]))
                    }
                    (false, true, _) => {
                        let coords = transform_input_coords(self, (new_x, new_y), false);
                        Event::TouchesUp(HashMap::from([(FingerId::VirtualCursor, coords)]))
                    }
                    (true, _, true) => {
                        let coords = transform_input_coords(self, (new_x, new_y), false);
                        Event::TouchesMove(HashMap::from([(FingerId::VirtualCursor, coords)]))
                    }
                    _ => return,
                });
        }
    }

    pub fn pop_event(&mut self) -> Option<Event> {
        self.high_priority_event
            .take()
            .or_else(|| self.event_queue.pop_front())
    }

    fn controller_added(&mut self, joystick_idx: u32) {
        let Ok(controller) = self.controller_ctx.open(joystick_idx) else {
            return;
        };

        let controller_name = controller.name();
        if env::consts::OS == "android" && controller_name.starts_with("uinput-") {
            return;
        }
        self.controllers.push(controller);
    }
    fn controller_removed(&mut self, instance_id: u32) {
        let Some(idx) = self
            .controllers
            .iter()
            .position(|controller| controller.instance_id() == instance_id)
        else {
            return;
        };
        self.controllers.remove(idx);
    }
    pub fn print_accelerometer_notice(&self, options: &Options) {
        if self.accelerometer.is_none() {
            log!(
                "You can {}hold right click and move the cursor to simulate the accelerometer.",
                if options.analog_stick_tilt_controls {
                    "also "
                } else {
                    ""
                }
            );
        }
    }

    pub fn get_acceleration(&self, options: &Options) -> (f32, f32, f32) {
        if self.controllers.is_empty() || !options.analog_stick_tilt_controls {
            if let Some(ref accelerometer) = self.accelerometer {
                let data = accelerometer.get_data().unwrap();
                let sdl2::sensor::SensorData::Accel(data) = data else {
                    panic!();
                };
                let [x, y, z] = data;
                let (x, y, z) = (-x, -y, -z);
                let gravity: f32 = 9.80665; 
                let (x, y, z) = (x / gravity, y / gravity, z / gravity);
                return (x, y, z);
            }
        }

        let (x, y) = if self
            .virtual_accelerometer_last
            .is_some_and(|(_x, _y, right_click_hold)| right_click_hold)
        {
            self.virtual_accelerometer_last
                .map(|(x, y, _right_click_hold)| (x, y))
                .unwrap()
        } else {
            let (x, y, _) = self.get_controller_stick(options, true);
            (x, y)
        };

        let [x, y] = self.rotation_matrix().inverse().unwrap().transform([x, y]);
        let (x, y) = (x.clamp(-1.0, 1.0), y.clamp(-1.0, 1.0)); 

        let gravity: [f32; 3] = [0.0, 0.0, -1.0];

        let neutral_x = options.x_tilt_offset.to_radians();
        let neutral_y = options.y_tilt_offset.to_radians();
        let x_rotation_range = options.x_tilt_range.to_radians() / 2.0;
        let y_rotation_range = options.y_tilt_range.to_radians() / 2.0;
        let x_rotation = neutral_x - x_rotation_range * y;
        let y_rotation = neutral_y - y_rotation_range * x;
        let matrix =
            Matrix::<3>::y_rotation(y_rotation).multiply(&Matrix::<3>::x_rotation(x_rotation));
        let [x, y, z] = matrix.transform(gravity);

        (x, y, z)
}
                    pub fn virtual_cursor_visible_at(&self) -> Option<(f32, f32, bool)> {
        let (x, y, pressed, visible) = self.virtual_cursor_last?;
        if visible {
            if let Some((x_unsticky, y_unsticky, _time)) = self.virtual_cursor_last_unsticky {
                Some((x_unsticky, y_unsticky, pressed))
            } else {
                Some((x, y, pressed))
            }
        } else {
            None
        }
    }

    fn update_virtual_cursor(&mut self, options: &Options) -> (f32, f32, bool, bool, bool) {
        let (x, y, pressed) = self.get_controller_stick(options, false);
        let visible = pressed || x != 0.0 || y != 0.0;

        let (vx, vy, vw, vh) = self.viewport();
        let (vx, vy, vw, vh) = (vx as f32, vy as f32, vw as f32, vh as f32);

        let (x, y) = {
            let ratio = vw / vh;
            let rect_height = (ratio * ratio + 1.0).powf(-0.5);
            let rect_width = ratio * rect_height;

            let x_abs = x.abs().min(rect_width) / rect_width;
            let y_abs = y.abs().min(rect_height) / rect_height;
            (x_abs.copysign(x), y_abs.copysign(y))
        };

        let x = (x / 2.0 + 0.5) * vw + vx;
        let y = (y / 2.0 + 0.5) * vh + vy;

        let (old_x, old_y, old_pressed, _old_visible) =
            self.virtual_cursor_last.unwrap_or_default();

        let (x, y) = if let Some((smoothing_strength, sticky_radius)) =
            options.stabilize_virtual_cursor
        {
            let new_time = Instant::now();
            let (old_x_unsticky, old_y_unsticky, old_time) = self
                .virtual_cursor_last_unsticky
                .unwrap_or((0.0, 0.0, new_time));

            let delta_t = new_time.saturating_duration_since(old_time).as_secs_f32();

            let smooth = |old: f32, new: f32| -> f32 {
                if smoothing_strength != 0.0 {
                    let lerp_factor = 1.0 - (0.5_f32).powf(delta_t * (1.0 / smoothing_strength));
                    old + (new - old) * lerp_factor
                } else {
                    new
                }
            };

            let new_x_unsticky = smooth(old_x_unsticky, x);
            let new_y_unsticky = smooth(old_y_unsticky, y);

            self.virtual_cursor_last_unsticky = Some((new_x_unsticky, new_y_unsticky, new_time));

            if (new_x_unsticky - old_x).hypot(new_y_unsticky - old_y) < sticky_radius {
                (old_x, old_y)
            } else {
                (new_x_unsticky, new_y_unsticky)
            }
        } else {
            (x, y)
        };

        self.virtual_cursor_last = Some((x, y, pressed, visible));

        (
            x,
            y,
            pressed,
            pressed != old_pressed,
            x != old_x || y != old_y,
        )
    }

    fn get_controller_stick(&self, options: &Options, left: bool) -> (f32, f32, bool) {
        fn convert_axis(axis: i16, deadzone: f32) -> f32 {
            assert!(deadzone >= 0.0);
            let axis = ((axis as f32) / (i16::MAX as f32)).clamp(-1.0, 1.0);
            let abs_axis = (axis.abs().max(deadzone) - deadzone) / (1.0 - deadzone);
            abs_axis.copysign(axis)
        }

        let (mut x, mut y) = (0.0, 0.0);
        let mut pressed = false;
        for controller in &self.controllers {
            use sdl2::controller::{Axis, Button};
            let (x_axis, y_axis, button1, button2) = if left {
                (
                    Axis::LeftX,
                    Axis::LeftY,
                    Button::LeftStick,
                    Button::LeftShoulder,
                )
            } else {
                (
                    Axis::RightX,
                    Axis::RightY,
                    Button::RightStick,
                    Button::RightShoulder,
                )
            };
            x += convert_axis(controller.axis(x_axis), options.deadzone);
            y += convert_axis(controller.axis(y_axis), options.deadzone);
            pressed |= controller.button(button1);
            pressed |= controller.button(button2);
        }
        let (x, y) = (x.clamp(-1.0, 1.0), y.clamp(-1.0, 1.0));

        (x, y, pressed)
    }

    pub fn create_gl_context(&self, version: GLVersion) -> Result<GLContext, String> {
        let attr = self.video_ctx.gl_attr();
        match version {
            GLVersion::GLES11 => {
                attr.set_context_version(1, 1);
                attr.set_context_profile(sdl2::video::GLProfile::GLES);
            }
            GLVersion::GLES20 => {
                attr.set_context_version(2, 0);
                attr.set_context_profile(sdl2::video::GLProfile::GLES);
            }
            GLVersion::GL21Compat => {
                attr.set_context_version(2, 1);
                attr.set_context_profile(sdl2::video::GLProfile::Compatibility);
            }
        }

        let gl_ctx = self.window.gl_create_context()?;
        Ok(GLContext(gl_ctx))
    }

    pub fn gl_get_proc_address(&self, procname: &str) -> *const std::ffi::c_void {
        self.video_ctx.gl_get_proc_address(procname) as *const _
    }

    pub fn set_share_with_current_context(&self, value: bool) {
        self.video_ctx
            .gl_attr()
            .set_share_with_current_context(value)
    }

    pub unsafe fn make_gl_context_current(&self, gl_ctx: &GLContext) {
        self.window.gl_make_current(&gl_ctx.0).unwrap();
    }

    #[must_use]
    pub fn make_internal_gl_ctx_current<'win>(&'win mut self) -> Box<dyn GLES + 'win> {
        let gl_ins = unsafe {
            self.internal_gl_ins
                .as_mut()
                .unwrap()
                .make_current_unchecked_for_window(
                    &mut |gl_ctx| self.window.gl_make_current(&gl_ctx.0).unwrap(),
                    &mut |s| self.video_ctx.gl_get_proc_address(s) as *const _,
                )
        };
        gl_ins
    }

    fn display_splash(&mut self) {
        assert!(self.splash_image.is_some());
        let matrix = self.rotation_matrix().multiply(&Matrix::y_flip());
        let (vx, vy, vw, vh) = self.viewport();
        let viewport = (vx, vy + self.viewport_y_offset(), vw, vh);

        let image = self.splash_image.as_ref().unwrap();

        unsafe {
            let mut gl_ctx = self
                .internal_gl_ins
                .as_mut()
                .unwrap()
                .make_current_unchecked_for_window(
                    &mut |gl_ctx| self.window.gl_make_current(&gl_ctx.0).unwrap(),
                    &mut |s| self.video_ctx.gl_get_proc_address(s) as *const _,
                );
            use crate::gles::gles11_raw as gles11; 

            let mut texture = 0;
            gl_ctx.GenTextures(1, &mut texture);
            gl_ctx.BindTexture(gles11::TEXTURE_2D, texture);
            let (width, height) = image.dimensions();
            gl_ctx.TexImage2D(
                gles11::TEXTURE_2D,
                0,
                gles11::RGBA as _,
                width as _,
                height as _,
                0,
                gles11::RGBA,
                gles11::UNSIGNED_BYTE,
                image.pixels().as_ptr() as *const _,
            );
            gl_ctx.TexParameteri(
                gles11::TEXTURE_2D,
                gles11::TEXTURE_MIN_FILTER,
                gles11::LINEAR as _,
            );
            gl_ctx.TexParameteri(
                gles11::TEXTURE_2D,
                gles11::TEXTURE_MAG_FILTER,
                gles11::LINEAR as _,
            );

            present_frame(
                gl_ctx.as_mut(),
                viewport,
                matrix,
                None,
            );

            gl_ctx.DeleteTextures(1, &texture);
        };

        self.window.gl_swap_window();
    }

    pub fn swap_window(&self) {
        self.window.gl_swap_window();
    }

    pub fn rotate_device(&mut self, new_orientation: DeviceOrientation) {
        if !self.on_main_stack {
            log!("Warning: rotate_device called off main stack, skipping");
            return;
        }
        if new_orientation == self.device_orientation {
            return;
        }

        if !self.fullscreen && !Self::rotatable_fullscreen() {
            let (width, height) = if Self::rotatable_fullscreen() {
                set_sdl2_orientation(new_orientation);
                rotate_fullscreen_size(new_orientation, self.window.size())
            } else {
                size_for_orientation(self.device_family, new_orientation, self.scale_hack)
            };

            #[cfg(target_os = "macos")]
            {
                let (_old_width, old_height) = self.window.size();
                self.max_height = self.max_height.max(old_height).max(height);
                self.viewport_y_offset = self.max_height - height;
            }

            self.window.set_size(width, height).unwrap();
        }

        if Self::rotatable_fullscreen() {
            set_sdl2_orientation(new_orientation);
            self.window
                .set_fullscreen(sdl2::video::FullscreenType::Off)
                .unwrap();
            unsafe {
                let window_raw = self.window.raw();
                sdl2_sys::SDL_SetWindowResizable(window_raw, sdl2_sys::SDL_bool::SDL_FALSE);
                sdl2_sys::SDL_SetWindowResizable(window_raw, sdl2_sys::SDL_bool::SDL_TRUE);
            }
            self.window
                .set_fullscreen(sdl2::video::FullscreenType::True)
                .unwrap();
        }

        self.device_orientation = new_orientation;

        if self.splash_image.is_some() {
            self.display_splash();
        }
    }

    pub fn device_family(&self) -> DeviceFamily {
        self.device_family
    }

    pub fn current_rotation(&self) -> DeviceOrientation {
        self.device_orientation
    }

    pub fn size_unrotated_unscaled(&self) -> (u32, u32) {
        size_for_orientation(
            self.device_family,
            DeviceOrientation::Portrait,
            NonZeroU32::new(1).unwrap(),
        )
    }

    pub fn viewport(&self) -> (u32, u32, u32, u32) {
        let (app_width, app_height) =
            size_for_orientation(self.device_family, self.device_orientation, self.scale_hack);
        if !self.fullscreen && !Self::rotatable_fullscreen() {
            return (0, 0, app_width, app_height);
        }

        let (screen_width, screen_height) = self.window.drawable_size();

        let app_aspect = app_width as f32 / app_height as f32;
        let screen_aspect = screen_width as f32 / screen_height as f32;
        let (scaled_width, scaled_height) = if app_aspect < screen_aspect {
            (
                (screen_height as f32 * app_aspect).round() as u32,
                screen_height,
            )
        } else {
            (
                screen_width,
                (screen_width as f32 / app_aspect).round() as u32,
            )
        };
        let x = (screen_width - scaled_width) / 2;
        let y = (screen_height - scaled_height) / 2;
        (x, y, scaled_width, scaled_height)
    }

    pub fn viewport_y_offset(&self) -> u32 {
        #[cfg(target_os = "macos")]
        return self.viewport_y_offset;
        #[cfg(not(target_os = "macos"))]
        return 0;
    }

    pub fn rotation_matrix(&self) -> Matrix<2> {
        match self.device_orientation {
            DeviceOrientation::Portrait => Matrix::identity(),
            DeviceOrientation::LandscapeLeft => Matrix::z_rotation(-FRAC_PI_2),
            DeviceOrientation::LandscapeRight => Matrix::z_rotation(FRAC_PI_2),
        }
    }

    pub fn is_screen_saver_enabled(&self) -> bool {
        self.video_ctx.is_screen_saver_enabled()
    }
    pub fn set_screen_saver_enabled(&mut self, enabled: bool) {
        if !self.on_main_stack {
            log!("Warning: set_screen_saver_enabled called off main stack, skipping");
            return;
        }
        match enabled {
            true => self.video_ctx.enable_screen_saver(),
            false => self.video_ctx.disable_screen_saver(),
        }
}
                   pub fn start_text_input(&self) {
        if !self.on_main_stack {
            log!("Warning: start_text_input called off main stack, skipping");
            return;
        }
        unsafe {
            sdl2_sys::SDL_StartTextInput();
        }
    }
    pub fn stop_text_input(&self) {
        if !self.on_main_stack {
            log!("Warning: stop_text_input called off main stack, skipping");
            return;
        }
        unsafe {
            sdl2_sys::SDL_StopTextInput();
        }
    }

    pub fn on_main_stack(&self) -> bool {
        self.on_main_stack
    }
}

pub fn open_url(env: &mut Environment, url: &str) -> Result<(), String> {
    env.on_parent_stack_in_coroutine(|_, _| sdl2::url::open_url(url).map_err(|e| e.to_string()))
}

pub fn show_error_messagebox(window: Option<&Window>, error_message: &str) {
    if window.is_some_and(|win| !win.on_main_stack) {
        log!("Warning: show_error_messagebox called off main stack, skipping");
        return;
    }
    use sdl2::messagebox;
    let mbox = [
        messagebox::ButtonData {
            flags: messagebox::MessageBoxButtonFlag::NOTHING,
            button_id: 0,
            text: "Open touchHLE directory",
        },
        messagebox::ButtonData {
            flags: messagebox::MessageBoxButtonFlag::NOTHING,
            button_id: 1,
            text: "Close",
        },
    ];

    let Ok(clicked_button) = messagebox::show_message_box(
        messagebox::MessageBoxFlag::ERROR,
        &mbox,
        "touchHLE crashed!",
        &format!("touchHLE crashed with the following error: {error_message}"),
        window.map(|win| &win.window),
        None,
    ) else {
        panic!("Failed to show message box!");
    };

    match clicked_button {
        messagebox::ClickedButton::CloseButton => {}
        messagebox::ClickedButton::CustomButton(button) => {
            match button.button_id {
                0 => match crate::paths::url_for_opening_user_data_dir() {
                    Ok(url) => {
                        if let Err(e) = sdl2::url::open_url(&url).map_err(|e| e.to_string()) {
                            echo!("Couldn't open file manager at {:?}: {}", url, e);
                        } else {
                            echo!("Opened file manager at {:?}, exiting.", url);
                        }
                    }
                    Err(e) => echo!("Couldn't open file manager: {}", e),
                },
                1 => {}
                _ => unreachable!(),
            }
        }
    }
}

pub fn get_battery_status() -> (i32, BatteryState) {
    let mut pct = 0;
    let status = unsafe { sdl2_sys::SDL_GetPowerInfo(null_mut(), &mut pct) };
    (
        pct,
        match status {
            SDL_PowerState::SDL_POWERSTATE_UNKNOWN => BatteryState::Unknown,
            SDL_PowerState::SDL_POWERSTATE_ON_BATTERY => BatteryState::OnBattery,
            SDL_PowerState::SDL_POWERSTATE_NO_BATTERY => BatteryState::NoBattery,
            SDL_PowerState::SDL_POWERSTATE_CHARGING => BatteryState::Charging,
            SDL_PowerState::SDL_POWERSTATE_CHARGED => BatteryState::Full,
        },
    )
}

pub fn get_preferred_language_codes(env: &mut Environment) -> Vec<String> {
    env.on_parent_stack_in_coroutine(|_, _| {
        sdl2::locale::get_preferred_locales()
            .map(|loc| loc.lang)
            .collect()
    })
}

pub fn get_preferred_country_codes(env: &mut Environment) -> Vec<String> {
    env.on_parent_stack_in_coroutine(|_, _| {
        sdl2::locale::get_preferred_locales()
            .filter_map(|loc| loc.country)
            .collect()
    })
}

pub fn show_alert_dialog(
    env: &mut Environment,
    title: &str,
    message: &str,
    buttons: &[&str],
) -> i32 {
    let title = title.to_string();
    let message = message.to_string();
    let buttons: Vec<String> = buttons.iter().map(|s| s.to_string()).collect();

    env.on_parent_stack_in_coroutine(move |window, _options| {
        use sdl2::messagebox;

        let button_data: Vec<messagebox::ButtonData> = buttons
            .iter()
            .enumerate()
            .map(|(i, text)| messagebox::ButtonData {
                flags: messagebox::MessageBoxButtonFlag::NOTHING,
                button_id: i as i32,
                text: text.as_str(),
            })
            .collect();

        match messagebox::show_message_box(
            messagebox::MessageBoxFlag::INFORMATION,
            &button_data,
            &title,
            &message,
            Some(&window.window),
            None,
        ) {
            Ok(messagebox::ClickedButton::CustomButton(btn)) => btn.button_id,
            _ => 0,
        }
    })
} 
