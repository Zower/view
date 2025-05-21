use std::num::NonZeroU32;

use femtovg::{renderer::OpenGl, Canvas};

use glutin::{
    config::ConfigTemplateBuilder,
    context::{ContextApi, ContextAttributesBuilder, NotCurrentContext},
    display::GetGlDisplay,
    prelude::*,
    surface::{SurfaceAttributesBuilder, WindowSurface},
};
use glutin_winit::DisplayBuilder;
use raw_window_handle::HasWindowHandle;
use winit::{
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Icon, WindowAttributes},
};

pub fn create_event_loop<T>(
    width: u32,
    height: u32,
    title: &'static str,
) -> (
    Canvas<OpenGl>,
    EventLoop<T>,
    glutin::context::PossiblyCurrentContext,
    glutin::surface::Surface<WindowSurface>,
    winit::window::Window,
    glutin::config::Config,
) {
    let event_loop = EventLoop::with_user_event().build().unwrap();

    let (canvas, context, surface, window, config) =
        create_gl_context_and_window(&event_loop, width, height, title);

    (canvas, event_loop, context, surface, window, config)
}

pub fn _new_window(
    event_loop: &ActiveEventLoop,
    width: u32,
    height: u32,
    title: &'static str,
    gl_config: &glutin::config::Config,
) -> (
    glutin::surface::Surface<WindowSurface>,
    winit::window::Window,
) {
    let image = include_bytes!("../../assets/icon.rgba");
    let icon = Icon::from_rgba(image.to_vec(), 1024, 1024).unwrap();

    let window_attr = WindowAttributes::default()
        .with_inner_size(winit::dpi::PhysicalSize::new(width, height))
        .with_resizable(true)
        .with_window_icon(Some(icon))
        .with_title(title);

    let window = glutin_winit::finalize_window(event_loop, window_attr, gl_config).unwrap();

    let raw_window_handle = window.window_handle().unwrap();

    let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
        raw_window_handle.as_raw(),
        NonZeroU32::new(width).unwrap(),
        NonZeroU32::new(height).unwrap(),
    );

    let surface = unsafe {
        gl_config
            .display()
            .create_window_surface(gl_config, &attrs)
            .unwrap()
    };

    (surface, window)
}

pub fn test(width: u32, height: u32) -> (EventLoop<()>, Canvas<OpenGl>, NotCurrentContext) {
    let event_loop = EventLoop::with_user_event().build().unwrap();

    let display_builder = DisplayBuilder::new().with_window_attributes(None);

    let template = ConfigTemplateBuilder::new().with_alpha_size(8);

    let (None, gl_config) = display_builder
        .build(&event_loop, template, |configs| {
            // Find the config with the maximum number of samples, so our triangle will
            // be smooth.
            configs
                .reduce(|accum, config| {
                    let transparency_check = config.supports_transparency().unwrap_or(false)
                        & !accum.supports_transparency().unwrap_or(false);

                    if transparency_check || config.num_samples() < accum.num_samples() {
                        config
                    } else {
                        accum
                    }
                })
                .unwrap()
        })
        .unwrap()
    else {
        panic!()
    };

    let gl_display = gl_config.display();

    let fallback_context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::Gles(None))
        .build(None);

    let not_current_gl_context = Some(unsafe {
        gl_display
            .create_context(&gl_config, &fallback_context_attributes)
            .unwrap()
    });

    // let gl_context = not_current_gl_context
    //     .take()
    //     .unwrap()
    //     .make_current(&surface)
    //     .unwrap();

    let renderer =
        unsafe { OpenGl::new_from_function_cstr(|s| gl_display.get_proc_address(s) as *const _) }
            .expect("Cannot create renderer");

    let mut canvas = Canvas::new(renderer).expect("Cannot create canvas");
    canvas.set_size(width, height, 1 as f32);

    (event_loop, canvas, not_current_gl_context.unwrap())
}

fn create_gl_context_and_window<T>(
    event_loop: &EventLoop<T>,
    width: u32,
    height: u32,
    title: &'static str,
) -> (
    Canvas<OpenGl>,
    glutin::context::PossiblyCurrentContext,
    glutin::surface::Surface<WindowSurface>,
    winit::window::Window,
    glutin::config::Config,
) {
    let image = include_bytes!("../../assets/icon.rgba");
    let icon = Icon::from_rgba(image.to_vec(), 1024, 1024).unwrap();

    let window_attrs = WindowAttributes::default()
        .with_inner_size(winit::dpi::PhysicalSize::new(width, height))
        .with_resizable(true)
        .with_visible(false)
        .with_window_icon(Some(icon))
        .with_title(title);

    let template = ConfigTemplateBuilder::new().with_alpha_size(8);

    let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attrs));

    let (window, gl_config) = display_builder
        .build(event_loop, template, |configs| {
            // Find the config with the maximum number of samples, so our triangle will
            // be smooth.
            configs
                .reduce(|accum, config| {
                    let transparency_check = config.supports_transparency().unwrap_or(false)
                        & !accum.supports_transparency().unwrap_or(false);

                    if transparency_check || config.num_samples() < accum.num_samples() {
                        config
                    } else {
                        accum
                    }
                })
                .unwrap()
        })
        .unwrap();

    let window = window.unwrap();

    let raw_window_handle = Some(window.window_handle().unwrap().as_raw());

    let gl_display = gl_config.display();

    let context_attributes = ContextAttributesBuilder::new().build(raw_window_handle);
    let fallback_context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::Gles(None))
        .build(raw_window_handle);

    let mut not_current_gl_context = Some(unsafe {
        gl_display
            .create_context(&gl_config, &context_attributes)
            .unwrap_or_else(|_| {
                gl_display
                    .create_context(&gl_config, &fallback_context_attributes)
                    .expect("failed to create context")
            })
    });

    let (width, height): (u32, u32) = window.inner_size().into();

    let raw_window_handle = window.window_handle().unwrap().as_raw();

    let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
        raw_window_handle,
        NonZeroU32::new(width).unwrap(),
        NonZeroU32::new(height).unwrap(),
    );

    let surface = unsafe {
        gl_config
            .display()
            .create_window_surface(&gl_config, &attrs)
            .unwrap()
    };

    let gl_context = not_current_gl_context
        .take()
        .unwrap()
        .make_current(&surface)
        .unwrap();

    surface
        .set_swap_interval(&gl_context, glutin::surface::SwapInterval::DontWait)
        .unwrap();

    let renderer =
        unsafe { OpenGl::new_from_function_cstr(|s| gl_display.get_proc_address(s) as *const _) }
            .expect("Cannot create renderer");

    let mut canvas = Canvas::new(renderer).expect("Cannot create canvas");
    canvas.set_size(width, height, window.scale_factor() as f32);

    (canvas, gl_context, surface, window, gl_config)
}
